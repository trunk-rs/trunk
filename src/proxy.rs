use std::sync::Arc;

use anyhow::Context;
use async_std::task::spawn;
use async_tungstenite::async_std::connect_async;
use futures::prelude::*;
use http_types::{Method, Url};
use tide::{Request, Result, Server};
use tide_websockets::{WebSocket, WebSocketConnection};

use crate::serve::State;

/// All HTTP methods, used for registering proxy endpoints with proper precedence.
static HTTP_METHODS: [Method; 9] = [
    Method::Get,
    Method::Head,
    Method::Post,
    Method::Put,
    Method::Delete,
    Method::Connect,
    Method::Options,
    Method::Trace,
    Method::Patch,
];

/// Proxy handler functionality.
pub trait ProxyHandler {
    /// The path on which this proxy handler is to listen.
    fn path(&self) -> &str;
    /// Register this proxy handler on the given app.
    fn register(self: Arc<Self>, app: &mut Server<State>);
}

/// A handler used for proxying HTTP requests to a backend.
pub struct ProxyHandlerHttp {
    /// The URL of the backend to which requests are to be proxied.
    backend: Url,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
}

impl ProxyHandler for ProxyHandlerHttp {
    fn path(&self) -> &str {
        self.rewrite
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or_else(|| self.backend.path())
    }

    fn register(self: Arc<Self>, app: &mut Server<State>) {
        // NOTE: we are using this loop instead of `.any` due to precedence issues in registering
        // routes, as described here https://github.com/thedodd/trunk/issues/95#issuecomment-753508639
        for method in HTTP_METHODS.iter() {
            let handler = self.clone();
            app.at(handler.path())
                .strip_prefix()
                .method(*method, move |req: Request<State>| {
                    let handler = handler.clone();
                    async move { handler.proxy_request(req).await }
                });
        }
    }
}

impl ProxyHandlerHttp {
    /// Create a new instance.
    pub fn new(backend: Url, rewrite: Option<String>) -> Self {
        Self { backend, rewrite }
    }

    /// Proxy the given request to the target backend.
    async fn proxy_request(&self, mut req: Request<State>) -> Result {
        // Prep the backend URL for proxied request.
        let req_url = req.url();
        let req_path = req_url.path();
        let mut url = self.backend.clone();
        if let Ok(mut segments) = url.path_segments_mut() {
            // Don't extend if empty.
            if req_path != "/" {
                segments.pop_if_empty().extend(req_path.trim_start_matches('/').split('/'));
            }
        }
        url.set_query(req_url.query());

        // Build a new request to be sent to the proxy backend.
        let mut request = surf::RequestBuilder::new(req.method(), url).body(req.take_body());
        for (hname, hval) in req.iter() {
            request = request.header(hname, hval);
        }
        // Ensure the host header is set to target the backend itself.
        if let Some(host) = self.backend.host_str() {
            request = request.header("host", host);
        }

        // Send the request & unpack the response.
        let mut res = request.send().await?;
        let mut response = tide::Response::builder(res.status()).body(res.take_body());
        for (hname, hval) in res.iter() {
            response = response.header(hname, hval);
        }
        Ok(response.build())
    }
}

/// A handler used for proxying WebSockets to a backend.
pub struct ProxyHandlerWebSocket {
    /// The URL of the backend to which requests are to be proxied.
    backend: Url,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
    /// An HTTP handler used for proxying requests which are not actually WebSocket related.
    http_handler: ProxyHandlerHttp,
}

impl ProxyHandler for ProxyHandlerWebSocket {
    fn path(&self) -> &str {
        self.rewrite
            .as_ref()
            .map(AsRef::as_ref)
            .unwrap_or_else(|| self.backend.path())
    }

    fn register(self: Arc<Self>, app: &mut Server<State>) {
        let handler = self.clone();
        app.at(self.path())
            .strip_prefix()
            .with(WebSocket::new(move |req, sock| self.clone().proxy_request(req, sock)))
            .get(move |req| {
                let handler = handler.clone();
                async move { handler.http_handler.proxy_request(req).await }
            });
    }
}

impl ProxyHandlerWebSocket {
    /// Create a new instance.
    pub fn new(backend: Url, rewrite: Option<String>) -> Self {
        let http_handler = ProxyHandlerHttp::new(backend.clone(), rewrite.clone());
        Self { backend, rewrite, http_handler }
    }

    /// Proxy the given request to the target backend.
    async fn proxy_request(self: Arc<Self>, req: Request<State>, frontend: WebSocketConnection) -> Result<()> {
        // Prep the backend URL for opening the backend WebSocket connection.
        let req_url = req.url();
        let req_path = req_url.path();
        let mut backend_url = self.backend.clone();
        if let Ok(mut segments) = backend_url.path_segments_mut() {
            // Don't extend if empty.
            if req_path != "/" {
                segments.pop_if_empty().extend(req_path.trim_start_matches('/').split('/'));
            }
        }

        // Open a WebSocket connection to the backend.
        let (mut backend_sink, mut backend_source) = connect_async(&backend_url)
            .await
            .with_context(|| format!("error establishing WebSocket connection to {:?}", backend_url))?
            .0
            .split();

        // Spawn a task for processing frontend messages.
        let mut frontend_source = frontend.clone();
        let frontend_handle = spawn(async move {
            while let Some(Ok(msg)) = frontend_source.next().await {
                if let Err(err) = backend_sink.send(msg).await {
                    eprintln!("error forwarding frontend WebSocket message to backend: {:?}", err);
                }
            }
        });

        // Spawn a task for processing backend messages.
        let backend_handle = spawn(async move {
            while let Some(Ok(msg)) = backend_source.next().await {
                if let Err(err) = frontend.send(msg).await {
                    eprintln!("error forwarding backend WebSocket message to frontend: {:?}", err);
                }
            }
        });

        futures::join!(frontend_handle, backend_handle);
        Ok(())
    }
}
