use crate::serve::{ServerError, ServerResult};
use anyhow::Context;
use axum::{
    body::Body,
    extract::{
        ws::{Message as MsgAxm, WebSocket, WebSocketUpgrade},
        Request, State,
    },
    http::{Response, Uri},
    routing::{any, get, Router},
    RequestExt,
};
use bytes::BytesMut;
use futures_util::{sink::SinkExt, stream::StreamExt, TryStreamExt};
use http::{header::HOST, HeaderMap};
use std::sync::Arc;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{protocol::CloseFrame, Message as MsgTng},
};
use tower_http::trace::TraceLayer;

/// The `X-Forwarded-Host`` (XFH) header is a de-facto standard header for
/// identifying the original host requested by the client in the Host HTTP
/// request header.
///
/// Refer: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Forwarded-Host
const X_FORWARDED_HOST: &str = "x-forwarded-host";
/// The X-Forwarded-Proto (XFP) header is a de-facto standard header for identifying the protocol
/// (HTTP or HTTPS) that a client used to connect to your proxy or load balancer.
///
/// Refer: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Forwarded-Proto
const X_FORWARDED_PROTO: &str = "x-forwarded-proto";

/// A handler used for proxying HTTP requests to a backend.
pub(crate) struct ProxyHandlerHttp {
    /// The protocol the proxy bound to
    proto: String,
    /// The client to use for proxy logic.
    client: reqwest::Client,
    /// The URL of the backend to which requests are to be proxied.
    backend: Uri,
    /// The headers to inject with the request
    request_headers: HeaderMap,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
}

fn make_outbound_uri(backend: &Uri, request: &Uri) -> anyhow::Result<Uri> {
    // 0, ensure the path always begins with `/`, this is required for a well-formed URI.
    // 1, the router always strips the value `state.path()`, so interpolate the backend path.
    // 2, optional "/" in case the backend path did not have a trailing slash.
    // 3, pass along the remaining path segment which was preserved by the router.
    let mut segments = ["/", "", "", "", "", ""];
    segments[1] = backend.path().trim_start_matches('/');
    segments[3] = request.path().trim_start_matches('/');

    // If the backend path is empty, we don't need another slash.
    // If the request path is empty, we don't need one either
    // If both are not empty, we need a slash to separate them.
    if !segments[1].is_empty() && !segments[3].is_empty() && !segments[1].ends_with('/') {
        segments[2] = "/";
    }

    // 4 & 5, pass along the query if applicable.
    if let Some(query) = request.query() {
        segments[4] = "?";
        segments[5] = query;
    }
    let path_and_query = segments.join("");

    // Construct the outbound URI & build a new request to be sent to the proxy backend.
    Uri::builder()
        .scheme(backend.scheme_str().unwrap_or_default())
        .authority(
            backend
                .authority()
                .map(|val| val.as_str())
                .unwrap_or_default(),
        )
        .path_and_query(path_and_query)
        .build()
        .context("error building proxy request to backend")
}

fn make_outbound_request(
    inbound_proto: &str,
    outbound_uri: &Uri,
    method: http::Method,
    original_headers: HeaderMap,
    override_headers: HeaderMap,
) -> anyhow::Result<http::request::Builder> {
    let mut request = http::Request::builder()
        .uri(outbound_uri.to_string())
        .method(method);

    // get the host header value from the outbound request

    let Some(outbound_host) = outbound_uri.authority().map(|authority| authority.host()) else {
        anyhow::bail!("No host found in outbound URI");
    };

    // forward all inbound headers

    for key in original_headers.keys() {
        let values = original_headers
            .get_all(key)
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        for value in values {
            if key == HOST {
                // Except for the host header, which we replace with the backend host value.
                // We also provide the original information in the XFH, XFP headers.
                request = request.header(HOST, outbound_host);
                request = request.header(X_FORWARDED_HOST, value);
                request = request.header(X_FORWARDED_PROTO, inbound_proto);
            } else {
                request = request.header(key, value);
            }
        }
    }

    // Apply all header overrides.
    // There is no special handling for any header (like host), as we leave manual intervention to
    // the user.

    if let Some(headers) = request.headers_mut() {
        for (key, value) in override_headers {
            let Some(key) = key else { continue };

            if value.is_empty() {
                // if the header value is empty, remove the header
                headers.remove(key);
            } else {
                // otherwise, replace header
                headers.insert(key, value);
            }
        }
    }

    Ok(request)
}

impl ProxyHandlerHttp {
    /// Construct a new instance.
    pub fn new(
        proto: String,
        client: reqwest::Client,
        backend: Uri,
        request_headers: HeaderMap,
        rewrite: Option<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            proto,
            client,
            backend,
            request_headers,
            rewrite,
        })
    }

    /// Build the sub-router for this proxy.
    pub fn register(self: Arc<Self>, router: Router) -> Router {
        router.nest_service(
            self.path(),
            any(Self::proxy_http_request)
                .layer(TraceLayer::new_for_http())
                .with_state(self.clone()),
        )
    }

    /// The path which this proxy backend listens at.
    pub fn path(&self) -> &str {
        self.rewrite
            .as_deref()
            .unwrap_or_else(|| self.backend.path())
    }

    /// Proxy the given request to the target backend.
    #[tracing::instrument(level = "debug", skip(state, req))]
    async fn proxy_http_request(
        State(state): State<Arc<Self>>,
        req: Request,
    ) -> ServerResult<Response<Body>> {
        // Construct the outbound URI & build a new request to be sent to the proxy backend.
        let outbound_uri = make_outbound_uri(&state.backend, req.uri())?;
        let outbound_req = make_outbound_request(
            &state.proto,
            &outbound_uri,
            req.method().clone(),
            req.headers().clone(),
            state.request_headers.clone(),
        )?;

        // set body
        let outbound_req = outbound_req
            .body(reqwest::Body::from(
                // It would be better to use a stream for this. However, right now,
                // .into_data_stream() returns a stream which is not Send+Sync, so we can't pass it
                // on to reqwest::Body::wrap_stream(..).
                req.into_body()
                    .into_data_stream()
                    .try_collect::<BytesMut>()
                    .await
                    .map_err(|err| ServerError(err.into()))?
                    .freeze(),
            ))
            .context("error building outbound request to proxy backend")?;

        // turn into reqwest type
        let outbound_req = outbound_req
            .try_into()
            .context("error translating outbound request")?;

        // Send the request & unpack the response.
        let backend_res = state
            .client
            .execute(outbound_req)
            .await
            .context("error proxying request to proxy backend")?;
        let mut res = Response::builder().status(backend_res.status());
        for (key, val) in backend_res.headers() {
            res = res.header(key, val);
        }

        Ok(res
            .body(Body::from_stream(backend_res.bytes_stream()))
            .context("error building proxy response")?)
    }
}

/// A handler used for proxying WebSockets to a backend.
pub struct ProxyHandlerWebSocket {
    /// The protocol the proxy bound to
    proto: String,
    /// The URL of the backend to which requests are to be proxied.
    backend: Uri,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
    /// The headers to inject with the request
    request_headers: HeaderMap,
}

impl ProxyHandlerWebSocket {
    /// Construct a new instance.
    pub fn new(
        proto: String,
        backend: Uri,
        headers: HeaderMap,
        rewrite: Option<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            proto,
            backend,
            rewrite,
            request_headers: headers,
        })
    }

    /// Build the sub-router for this proxy.
    pub fn register(self: Arc<Self>, router: Router) -> Router {
        let proxy = self.clone();
        let override_headers = self.request_headers.clone();
        let proto = self.proto.clone();
        router.nest_service(
            self.path(),
            get(|req: Request<Body>| async move {
                let req_headers = req.headers().to_owned();
                let uri = req.uri().clone();
                let ws = req.extract::<WebSocketUpgrade, _>().await;
                ws.map(|e| {
                    e.on_upgrade(|socket| async move {
                        proxy
                            .clone()
                            .proxy_ws_request(&proto, socket, uri, req_headers, override_headers)
                            .await
                    })
                })
            }),
        )
    }

    /// The path which this proxy backend listens at.
    pub fn path(&self) -> &str {
        self.rewrite
            .as_deref()
            .unwrap_or_else(|| self.backend.path())
    }

    /// Proxy the given WebSocket request to the target backend.
    #[tracing::instrument(level = "debug", skip(self, ws))]
    async fn proxy_ws_request(
        self: Arc<Self>,
        inbound_proto: &str,
        ws: WebSocket,
        request_uri: Uri,
        req_headers: HeaderMap,
        override_headers: HeaderMap,
    ) {
        tracing::debug!("new websocket connection");

        // Build where request will be forwarded
        let outbound_uri = match make_outbound_uri(&self.backend, &request_uri) {
            Ok(outbound_uri) => outbound_uri,
            Err(err) => {
                tracing::error!(error = ?err, "failed to build proxy uri from {:?}", &request_uri);
                return;
            }
        };

        let outbound_request = match make_outbound_request(
            inbound_proto,
            &outbound_uri,
            http::Method::GET,
            req_headers,
            override_headers,
        ) {
            Ok(outbound_uri) => outbound_uri,
            Err(err) => {
                tracing::error!(error = ?err, "failed to create outbound request");
                return;
            }
        };

        let outbound_request = match outbound_request
            .body(())
            .context("Failed to build outbound request")
        {
            Ok(outbound_uri) => outbound_uri,
            Err(err) => {
                tracing::error!(error = ?err, "failed to build outbound request");
                return;
            }
        };

        // Establish WS connection to backend.
        let (backend, _res) = match connect_async(outbound_request).await {
            Ok(backend) => backend,
            Err(err) => {
                tracing::error!(error = ?err, "error establishing WebSocket connection to backend {:?} for proxy", &outbound_uri);
                return;
            }
        };
        let (mut backend_sink, mut backend_stream) = backend.split();
        let (mut frontend_sink, mut frontend_stream) = ws.split();

        // Stream frontend messages to backend.
        let stream_to_backend = async move {
            while let Some(Ok(msg_axm)) = frontend_stream.next().await {
                let msg_tng = match msg_axm {
                    MsgAxm::Text(msg) => MsgTng::Text(msg.as_str().into()),
                    MsgAxm::Binary(msg) => MsgTng::Binary(msg),
                    MsgAxm::Ping(msg) => MsgTng::Ping(msg),
                    MsgAxm::Pong(msg) => MsgTng::Pong(msg),
                    MsgAxm::Close(Some(close_frame)) => MsgTng::Close(Some(CloseFrame {
                        code: close_frame.code.into(),
                        reason: close_frame.reason.as_str().into(),
                    })),
                    MsgAxm::Close(None) => MsgTng::Close(None),
                };

                if let Err(err) = backend_sink.send(msg_tng).await {
                    tracing::error!(error = ?err, "error forwarding frontend WebSocket message to backend");
                    return;
                }
            }
        };

        // Stream backend messages to frontend.
        let stream_to_frontend = async move {
            while let Some(Ok(msg)) = backend_stream.next().await {
                let msg_axm = match msg {
                    MsgTng::Binary(val) => MsgAxm::Binary(val),
                    MsgTng::Text(val) => MsgAxm::Text(val.as_str().into()),
                    MsgTng::Ping(val) => MsgAxm::Ping(val),
                    MsgTng::Pong(val) => MsgAxm::Pong(val),
                    MsgTng::Close(Some(frame)) => {
                        MsgAxm::Close(Some(axum::extract::ws::CloseFrame {
                            code: frame.code.into(),
                            reason: frame.reason.as_str().into(),
                        }))
                    }
                    MsgTng::Close(None) => MsgAxm::Close(None),
                    MsgTng::Frame(_) => continue,
                };
                if let Err(err) = frontend_sink.send(msg_axm).await {
                    tracing::error!(error = ?err, "error forwarding backend WebSocket message to frontend");
                    return;
                }
            }
        };

        tokio::select! {
            _ = stream_to_backend => (),
            _ = stream_to_frontend => ()
        };

        tracing::debug!("websocket connection closed");
    }
}

#[cfg(test)]
mod tests {
    use crate::proxy::make_outbound_uri;
    use axum::http::{HeaderValue, Uri};
    use http::{
        header::{
            ACCEPT, ACCEPT_ENCODING, CONNECTION, CONTENT_LENGTH, CONTENT_TYPE, COOKIE, DATE,
            EXPECT, HOST, USER_AGENT,
        },
        HeaderMap,
    };

    use super::{make_outbound_request, X_FORWARDED_HOST};

    #[test]
    fn make_outbound_uri_two_base_paths() {
        let backend = Uri::from_static("https://backend/");
        let request = Uri::from_static("http://localhost/");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/")
        )
    }

    #[test]
    fn make_outbound_uri_two_empty_paths() {
        let backend = Uri::from_static("https://backend");
        let request = Uri::from_static("http://localhost");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/")
        )
    }

    #[test]
    fn make_outbound_uri_two_with_query() {
        let backend = Uri::from_static("https://backend/");
        let request = Uri::from_static("http://localhost/auth?user=user&pwd=secret");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/auth?user=user&pwd=secret")
        )
    }

    #[test]
    fn make_outbound_uri_two_slash_at_end() {
        let backend = Uri::from_static("https://backend/");
        let request = Uri::from_static("http://localhost/auth/");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/auth/")
        )
    }

    #[test]
    fn make_outbound_uri_request_with_path() {
        let backend = Uri::from_static("https://backend/");
        let request = Uri::from_static("http://localhost/auth");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/auth")
        )
    }

    #[test]
    fn make_outbound_uri_request_with_sub_paths() {
        let backend = Uri::from_static("https://backend/sub");
        let request = Uri::from_static("http://localhost/auth");
        assert_eq!(
            make_outbound_uri(&backend, &request).expect("Unexpected error"),
            Uri::from_static("https://backend/sub/auth")
        )
    }

    #[test]
    fn make_outbound_request_from_uri_and_headers() {
        let backend_uri = Uri::from_static("https://backend/sub");
        let inbound_uri = Uri::from_static("http://localhost/auth");
        let inbound_headers = vec![
            (
                HOST,
                HeaderValue::from_str("localhost").expect("Failed to create Header Value"),
            ),
            (
                USER_AGENT,
                HeaderValue::from_str("curl/7.64.1").expect("Failed to create Header Value"),
            ),
            (
                ACCEPT,
                HeaderValue::from_str("*/*").expect("Failed to create Header Value"),
            ),
            (
                ACCEPT_ENCODING,
                HeaderValue::from_str("deflate, gzip").expect("Failed to create Header Value"),
            ),
            (
                CONNECTION,
                HeaderValue::from_str("keep-alive").expect("Failed to create Header Value"),
            ),
            (
                CONTENT_LENGTH,
                HeaderValue::from_str("0").expect("Failed to create Header Value"),
            ),
            (
                CONTENT_TYPE,
                HeaderValue::from_str("application/json").expect("Failed to create Header Value"),
            ),
            (
                DATE,
                HeaderValue::from_str("Tue, 01 Dec 2020 00:00:00 GMT")
                    .expect("Failed to create Header Value"),
            ),
            (
                EXPECT,
                HeaderValue::from_str("").expect("Failed to create Header Value"),
            ),
            (
                COOKIE,
                HeaderValue::from_str("cookie1=value1; cookie2=value2")
                    .expect("Failed to create Header Value"),
            ),
            (
                COOKIE,
                HeaderValue::from_str("cookie3=value1; cookie4=value2")
                    .expect("Failed to create Header Value"),
            ),
        ];
        let mut want_headers = HeaderMap::new();

        for (key, val) in inbound_headers {
            want_headers.append(key, val);
        }

        let have_outbound_uri = make_outbound_uri(&backend_uri, &inbound_uri)
            .expect("Failed to create Uri instance from inbound");
        let have_outbound_req = make_outbound_request(
            "http",
            &have_outbound_uri,
            http::Method::GET,
            want_headers.clone(),
            Default::default(),
        )
        .expect("Failed to create Request instance from inbound")
        .body(())
        .expect("Failed to create Request from builder");

        assert_eq!(have_outbound_req.uri(), &have_outbound_uri);
        assert_eq!(have_outbound_req.method(), &http::Method::GET);
        assert_eq!(
            have_outbound_req
                .headers()
                .get(HOST)
                .expect("Expected HOST header"),
            &HeaderValue::from_static("backend")
        );

        for key in want_headers.keys() {
            if key == HOST {
                continue;
            }

            if key == X_FORWARDED_HOST {
                assert_eq!(
                    have_outbound_req
                        .headers()
                        .get(key.clone())
                        .unwrap_or_else(|| panic!("Expected header value for {}", key)),
                    &HeaderValue::from_static("localhost")
                );
                continue;
            }

            let val = want_headers.get_all(key).iter().collect::<Vec<_>>();

            assert_eq!(
                have_outbound_req
                    .headers()
                    .get_all(key.clone())
                    .iter()
                    .collect::<Vec<_>>(),
                val
            );
        }
    }
}
