use std::sync::Arc;

use anyhow::Context;
use axum::routing::BoxRoute;
use axum::ws::{ws, Message as MsgAxm, WebSocket};
use axum::{prelude::*, AddExtensionLayer};
use futures::prelude::*;
use http::Uri;
use hyper::{Body, Request, Response};
use reqwest::header::HeaderValue;
use tokio_tungstenite::{connect_async, tungstenite::Message as MsgTng};
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

impl ProxyHandlerHttp {
    /// Construct a new instance.
    pub fn new(client: reqwest::Client, backend: Uri, rewrite: Option<String>) -> Arc<Self> {
        Arc::new(Self { client, backend, rewrite })
    }

    /// Build the sub-router for this proxy.
    pub fn register(self: Arc<Self>, router: BoxRoute<Body>) -> BoxRoute<Body> {
        router
            .nest(
                self.path(),
                any(Self::proxy_http_request
                    .layer(AddExtensionLayer::new(self.clone()))
                    .layer(TraceLayer::new_for_http())),
            )
            .boxed()
    }

    /// The path which this proxy backend listens at.
    pub fn path(&self) -> &str {
        self.rewrite.as_deref().unwrap_or_else(|| self.backend.path())
    }

    /// Proxy the given request to the target backend.
    #[tracing::instrument(level = "debug", skip(req))]
    async fn proxy_http_request(req: Request<Body>) -> ServerResult<Response<Body>> {
        let state = req
            .extensions()
            .get::<Arc<Self>>()
            .cloned()
            .context("error accessing proxy handler state")?;

        // 0, ensure the path always begins with `/`, this is required for a well-formed URI.
        // 1, the router always strips the value `state.path()`, so interpolate the backend path.
        // 2, pass along the remaining path segment which was preserved by the router.
        let mut segments = ["/", "", "", "", ""];
        segments[1] = state.backend.path().trim_start_matches('/');
        if state.backend.path().ends_with('/') {
            segments[2] = req.uri().path().trim_start_matches('/');
        } else {
            segments[2] = req.uri().path();
        }
        // 3 & 4, pass along the query if applicable.
        if let Some(query) = req.uri().query() {
            segments[3] = "?";
            segments[4] = query;
        }
        let path_and_query = segments.join("");

        // Construct the outbound URI & build a new request to be sent to the proxy backend.
        let outbound_uri = Uri::builder()
            .scheme(state.backend.scheme_str().unwrap_or_default())
            .authority(state.backend.authority().map(|val| val.as_str()).unwrap_or_default())
            .path_and_query(path_and_query)
            .build()
            .context("error building proxy request to backend")?;
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
        let mut res = http::Response::builder().status(backend_res.status());
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
    pub fn register(self: Arc<Self>, router: BoxRoute<Body>) -> BoxRoute<Body> {
        let proxy = self.clone();
        router
            .route(
                self.path(),
                ws(move |ws: WebSocket| async move { proxy.clone().proxy_ws_request(ws).await }),
            )
            .boxed()
    }

    /// The path which this proxy backend listens at.
    pub fn path(&self) -> &str {
        self.rewrite.as_deref().unwrap_or_else(|| self.backend.path())
    }

    /// Proxy the given WebSocket request to the target backend.
    #[tracing::instrument(level = "debug", skip(self, ws))]
    async fn proxy_ws_request(self: Arc<Self>, ws: WebSocket) {
        tracing::debug!("new websocket connection");

        // Establish WS connection to backend.
        let (backend, _res) = match connect_async(self.backend.clone()).await {
            Ok(backend) => backend,
            Err(err) => {
                tracing::error!(error = ?err, "error establishing WebSocket connection to backend {:?} for proxy", &self.backend);
                return;
            }
        };
        let (mut backend_sink, mut backend_stream) = backend.split();
        let (mut frontend_sink, mut frontend_stream) = ws.split();

        // Stream frontend messages to backend.
        let stream_to_backend = async move {
            while let Some(Ok(msg)) = frontend_stream.next().await {
                if let Err(err) = backend_sink.send(MsgTng::from(msg.as_bytes())).await {
                    tracing::error!(error = ?err, "error forwarding frontend WebSocket message to backend");
                    return;
                }
            }
        };

        // Stream backend messages to frontend.
        let stream_to_frontend = async move {
            while let Some(Ok(msg)) = backend_stream.next().await {
                let msg_axm = match msg {
                    MsgTng::Binary(val) => MsgAxm::binary(val),
                    MsgTng::Text(val) => MsgAxm::text(val),
                    MsgTng::Ping(val) => MsgAxm::ping(val),
                    MsgTng::Pong(val) => MsgAxm::pong(val),
                    MsgTng::Close(Some(frame)) => MsgAxm::close_with(frame.code, frame.reason),
                    MsgTng::Close(None) => MsgAxm::close(),
                };
                if let Err(err) = frontend_sink.send(msg_axm).await {
                    tracing::error!(error = ?err, "error forwarding backend WebSocket message to frontend");
                    return;
                }
            }
        };

        futures::join!(stream_to_backend, stream_to_frontend);
        tracing::debug!("websocket connection closed");
    }
}
