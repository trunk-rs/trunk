use std::sync::Arc;

use anyhow::Context;
use axum::body::Body;
use axum::extract::ws::{Message as MsgAxm, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{Request, Response, Uri};
use axum::routing::{any, get, Router};
use axum::RequestExt;
use futures_util::sink::SinkExt;
use futures_util::stream::StreamExt;
use hyper::header::HOST;
use hyper::HeaderMap;
use reqwest::header::HeaderValue;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message as MsgTng;
use tower_http::trace::TraceLayer;

use crate::serve::ServerResult;

/// A handler used for proxying HTTP requests to a backend.
pub(crate) struct ProxyHandlerHttp {
    /// The client to use for proxy logic.
    client: reqwest::Client,
    /// The URL of the backend to which requests are to be proxied.
    backend: Uri,
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
    outbound_uri: &Uri,
    headers: HeaderMap,
) -> anyhow::Result<hyper::Request<()>> {
    let mut request = hyper::Request::builder().uri(outbound_uri.to_string());

    let Some(host) = outbound_uri.authority().map(|authority| authority.host()) else {
        anyhow::bail!("No host found in outbound URI");
    };

    for (maybe_key, val) in headers {
        if let Some(key) = maybe_key {
            if key == HOST {
                request = request.header(HOST, host);
            } else {
                request = request.header(key, val);
            }
        }
    }

    request.body(()).context("Failed to build outbound request")
}

impl ProxyHandlerHttp {
    /// Construct a new instance.
    pub fn new(client: reqwest::Client, backend: Uri, rewrite: Option<String>) -> Arc<Self> {
        Arc::new(Self {
            client,
            backend,
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
        req: Request<Body>,
    ) -> ServerResult<Response<Body>> {
        // Construct the outbound URI & build a new request to be sent to the proxy backend.
        let outbound_uri = make_outbound_uri(&state.backend, req.uri())?;
        let mut outbound_req = state
            .client
            .request(req.method().clone(), outbound_uri.to_string())
            .headers(req.headers().clone())
            .body(req.into_body())
            .build()
            .context("error building outbound request to proxy backend")?;

        // Ensure the host header is set to target the backend.
        if let Some(host) = state.backend.authority().map(|authority| authority.host()) {
            if let Ok(host) = HeaderValue::from_str(host) {
                outbound_req.headers_mut().insert("host", host);
            }
        }

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
            .body(Body::wrap_stream(backend_res.bytes_stream()))
            .context("error building proxy response")?)
    }
}

/// A handler used for proxying WebSockets to a backend.
pub struct ProxyHandlerWebSocket {
    /// The URL of the backend to which requests are to be proxied.
    backend: Uri,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
}

impl ProxyHandlerWebSocket {
    /// Construct a new instance.
    pub fn new(backend: Uri, rewrite: Option<String>) -> Arc<Self> {
        Arc::new(Self { backend, rewrite })
    }

    /// Build the sub-router for this proxy.
    pub fn register(self: Arc<Self>, router: Router) -> Router {
        let proxy = self.clone();
        router.nest_service(
            self.path(),
            get(|req: Request<Body>| async move {
                let headers = req.headers().to_owned();
                let uri = req.uri().clone();
                let ws = req.extract::<WebSocketUpgrade, _>().await;
                ws.map(|e| {
                    e.on_upgrade(|socket| async move {
                        proxy.clone().proxy_ws_request(socket, uri, headers).await
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
        ws: WebSocket,
        request_uri: Uri,
        headers: HeaderMap,
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

        let outbound_request = match make_outbound_request(&outbound_uri, headers) {
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
                    MsgAxm::Text(msg) => MsgTng::Text(msg),
                    MsgAxm::Binary(msg) => MsgTng::Binary(msg),
                    MsgAxm::Ping(msg) => MsgTng::Ping(msg),
                    MsgAxm::Pong(msg) => MsgTng::Pong(msg),
                    MsgAxm::Close(Some(close_frame)) => MsgTng::Close(Some(CloseFrame {
                        code: close_frame.code.into(),
                        reason: close_frame.reason,
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
                    MsgTng::Text(val) => MsgAxm::Text(val),
                    MsgTng::Ping(val) => MsgAxm::Ping(val),
                    MsgTng::Pong(val) => MsgAxm::Pong(val),
                    MsgTng::Close(Some(frame)) => {
                        MsgAxm::Close(Some(axum::extract::ws::CloseFrame {
                            code: frame.code.into(),
                            reason: frame.reason,
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
    use axum::http::Uri;

    use crate::proxy::make_outbound_uri;

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
}
