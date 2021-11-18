use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::{
    ws::{WebSocket, WebSocketUpgrade},
    Extension,
};
use axum::handler::{get, Handler};
use axum::routing::{BoxRoute, Router};
use axum::AddExtensionLayer;
use hyper::{Body, Request, Response, Server, StatusCode};
use hyper_staticfile::{resolve_path, ResolveResult, ResponseBuilder};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::trace::TraceLayer;

use crate::common::SERVER;
use crate::config::RtcServe;
use crate::proxy::{ProxyHandlerHttp, ProxyHandlerWebSocket};
use crate::watch::WatchSystem;

const INDEX_HTML: &str = "index.html";

/// A system encapsulating a build & watch system, responsible for serving generated content.
pub struct ServeSystem {
    cfg: Arc<RtcServe>,
    watch: WatchSystem,
    http_addr: String,
    shutdown_tx: broadcast::Sender<()>,
    //  N.B. we use a broadcast channel here because a watch channel triggers a
    //  false positive on the first read of channel
    build_done_chan: broadcast::Sender<()>,
}

impl ServeSystem {
    /// Construct a new instance.
    pub async fn new(cfg: Arc<RtcServe>, shutdown: broadcast::Sender<()>) -> Result<Self> {
        let (build_done_chan, _) = broadcast::channel(8);
        let watch = WatchSystem::new(cfg.watch.clone(), shutdown.clone(), Some(build_done_chan.clone())).await?;
        let http_addr = format!("http://{}:{}{}", cfg.address, cfg.port, &cfg.watch.build.public_url);
        Ok(Self {
            cfg,
            watch,
            http_addr,
            shutdown_tx: shutdown,
            build_done_chan,
        })
    }

    /// Run the serve system.
    #[tracing::instrument(level = "trace", skip(self))]
    pub async fn run(mut self) -> Result<()> {
        // Spawn the watcher & the server.
        let _build_res = self.watch.build().await; // TODO: only open after a successful build.
        let watch_handle = tokio::spawn(self.watch.run());
        let server_handle = Self::spawn_server(self.cfg.clone(), self.shutdown_tx.subscribe(), self.build_done_chan)?;

        // Open the browser.
        if self.cfg.open {
            if let Err(err) = open::that(self.http_addr) {
                tracing::error!(error = ?err, "error opening browser");
            }
        }
        drop(self.shutdown_tx); // Drop the broadcast channel to ensure it does not keep the system alive.
        if let Err(err) = watch_handle.await {
            tracing::error!(error = ?err, "error joining watch system handle");
        }
        if let Err(err) = server_handle.await {
            tracing::error!(error = ?err, "error joining server handle");
        }
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(cfg, shutdown_rx))]
    fn spawn_server(cfg: Arc<RtcServe>, mut shutdown_rx: broadcast::Receiver<()>, build_done_chan: broadcast::Sender<()>) -> Result<JoinHandle<()>> {
        // Build a shutdown signal for the warp server.
        let shutdown_fut = async move {
            // Any event on this channel, even a drop, should trigger shutdown.
            let _res = shutdown_rx.recv().await;
            tracing::debug!("server is shutting down");
        };

        // Build the proxy client.
        let client = reqwest::ClientBuilder::new()
            .build()
            .context("error building proxy client")?;

        // Build the server.
        let state = Arc::new(State::new(
            cfg.watch.build.final_dist.clone(),
            cfg.watch.build.public_url.clone(),
            client,
            &cfg,
            build_done_chan,
        ));
        let router = router(state, cfg.clone());
        let addr = format!("{}:{}", cfg.address, cfg.port).parse()?;
        let server = Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(shutdown_fut);

        // Block this routine on the server's completion.
        tracing::info!("{} server listening at {}", SERVER, addr);
        Ok(tokio::spawn(async move {
            if let Err(err) = server.await {
                tracing::error!(error = ?err, "error from server task");
            }
        }))
    }
}

/// Server state.
pub struct State {
    /// A client instance used by proxies.
    pub client: reqwest::Client,
    /// The location of the dist dir.
    pub dist_dir: PathBuf,
    /// The public URL from which assets are being served.
    pub public_url: String,
    /// The channel to receive build_done notifications on.
    pub build_done_chan: broadcast::Sender<()>,
    /// Whether to disable autoreload
    pub no_autoreload: bool,
}

impl State {
    /// Construct a new instance.
    pub fn new(dist_dir: PathBuf, public_url: String, client: reqwest::Client, cfg: &RtcServe, build_done_chan: broadcast::Sender<()>) -> Self {
        Self {
            client,
            dist_dir,
            public_url,
            build_done_chan,
            no_autoreload: cfg.no_autoreload,
        }
    }
}

/// Serve the static dist dir.
#[tracing::instrument(level = "debug", skip(req))]
async fn serve_dist(req: Request<Body>) -> ServerResult<Response<Body>> {
    let state = req
        .extensions()
        .get::<Arc<State>>()
        .context("error accessing request state")?;
    let accept_header_opt = req.headers().get("accept").map(|val| val.to_str());
    let res = resolve_path(state.dist_dir.as_path(), req.uri().path())
        .await
        .context("error serving from dist dir")?;

    // If the target file was not found, we have an accept header, and that accept header allows
    // for HTML to be returned, then move on to attempt to serve the index.html. Else, respond.
    match (&res, accept_header_opt) {
        // If accept does not contain `*/*` or `text/html`, then return.
        (ResolveResult::NotFound, Some(Ok(accept_header))) if accept_header.contains("*/*") || accept_header.contains("text/html") => (),
        _ => {
            return Ok(ResponseBuilder::new()
                .request(&req)
                .build(res)
                .context("error serving from dist dir")?)
        }
    };

    // At this point, we have a 404 with an accept header allowing HTML, so attempt to serve the index.
    let res = resolve_path(state.dist_dir.as_path(), INDEX_HTML)
        .await
        .context("error serving index.html from dist dir")?;
    Ok(ResponseBuilder::new()
        .request(&req)
        .build(res)
        .context("error serving index.html from dist dir")?)
}

/// Build the Trunk router, this includes that static file server, the WebSocket server,
/// (for autoreload & HMR in the future), as well as any user-defined proxies.
fn router(state: Arc<State>, cfg: Arc<RtcServe>) -> Router<BoxRoute> {
    // Build static file server, middleware, error handler & WS route for reloads.
    let mut router = Router::new()
        .nest(&state.public_url, get(serve_dist.layer(TraceLayer::new_for_http())))
        .route(
            "/_trunk/ws",
            get(|ws: WebSocketUpgrade, state: Extension<Arc<State>>| async move {
                ws.on_upgrade(|socket| async move { handle_ws(socket, state.0).await })
            }),
        )
        .layer(AddExtensionLayer::new(state.clone()))
        .boxed();

    tracing::info!("{} serving static assets at -> {}", SERVER, state.public_url.as_str());

    // Build proxies.
    if let Some(backend) = &cfg.proxy_backend {
        if cfg.proxy_ws {
            let handler = ProxyHandlerWebSocket::new(backend.clone(), cfg.proxy_rewrite.clone());
            router = handler.clone().register(router);
            tracing::info!("{} proxying websocket {} -> {}", SERVER, handler.path(), &backend);
        } else {
            let handler = ProxyHandlerHttp::new(state.client.clone(), backend.clone(), cfg.proxy_rewrite.clone());
            router = handler.clone().register(router);
            tracing::info!("{} proxying {} -> {}", SERVER, handler.path(), &backend);
        }
    } else if let Some(proxies) = &cfg.proxies {
        for proxy in proxies.iter() {
            if proxy.ws {
                let handler = ProxyHandlerWebSocket::new(proxy.backend.clone(), proxy.rewrite.clone());
                router = handler.clone().register(router);
                tracing::info!("{} proxying websocket {} -> {}", SERVER, handler.path(), &proxy.backend);
            } else {
                let handler = ProxyHandlerHttp::new(state.client.clone(), proxy.backend.clone(), proxy.rewrite.clone());
                router = handler.clone().register(router);
                tracing::info!("{} proxying {} -> {}", SERVER, handler.path(), &proxy.backend);
            };
        }
    }

    router
}

async fn handle_ws(mut ws: WebSocket, state: Arc<State>) {
    let mut rx = state.build_done_chan.subscribe();
    tracing::debug!("autoreload websocket opened");
    while tokio::select! {
        _ = ws.recv() => {
            tracing::debug!("autoreload websocket closed");
            return
        }
        build_done = rx.recv() => build_done.is_ok(),
    } {
        let ws_send = ws.send(axum::extract::ws::Message::Text(r#"{"reload": true}"#.to_owned()));
        if ws_send.await.is_err() {
            break;
        }
    }
}

/// A result type used to work seamlessly with axum.
pub(crate) type ServerResult<T> = std::result::Result<T, ServerError>;

/// A newtype to make anyhow errors work with axum.
pub(crate) struct ServerError(pub anyhow::Error);

impl From<anyhow::Error> for ServerError {
    fn from(src: anyhow::Error) -> Self {
        ServerError(src)
    }
}

impl axum::response::IntoResponse for ServerError {
    type Body = Body;
    type BodyError = <Self::Body as axum::body::HttpBody>::Error;

    fn into_response(self) -> Response<Body> {
        tracing::error!(error = ?self.0, "error handling request");
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        res
    }
}
