use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::{self, Body};
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::extract::Extension;
use axum::http::header::HeaderName;
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{get, get_service, Router};
use axum::Server;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
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
        let watch = WatchSystem::new(
            cfg.watch.clone(),
            shutdown.clone(),
            Some(build_done_chan.clone()),
        )
        .await?;
        let http_addr = format!(
            "http://{}:{}{}",
            cfg.address, cfg.port, &cfg.watch.build.public_url
        );
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
        let server_handle = Self::spawn_server(
            self.cfg.clone(),
            self.shutdown_tx.subscribe(),
            self.build_done_chan,
        )?;

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
    fn spawn_server(
        cfg: Arc<RtcServe>,
        mut shutdown_rx: broadcast::Receiver<()>,
        build_done_chan: broadcast::Sender<()>,
    ) -> Result<JoinHandle<()>> {
        // Build a shutdown signal for the warp server.
        let shutdown_fut = async move {
            // Any event on this channel, even a drop, should trigger shutdown.
            let _res = shutdown_rx.recv().await;
            tracing::debug!("server is shutting down");
        };

        // Build the proxy client.
        let client = reqwest::ClientBuilder::new()
            .http1_only()
            .build()
            .context("error building proxy client")?;

        let insecure_client = reqwest::ClientBuilder::new()
            .http1_only()
            .danger_accept_invalid_certs(true)
            .build()
            .context("error building insecure proxy client")?;

        // Build the server.
        let state = Arc::new(State::new(
            cfg.watch.build.final_dist.clone(),
            cfg.watch.build.public_url.clone(),
            client,
            insecure_client,
            &cfg,
            build_done_chan,
        ));
        let router = router(state, cfg.clone())?;
        let addr = (cfg.address, cfg.port).into();
        let server = Server::bind(&addr)
            .serve(router.into_make_service())
            .with_graceful_shutdown(shutdown_fut);

        // Block this routine on the server's completion.
        tracing::info!("{} server listening at http://{}", SERVER, addr);
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
    /// A client instance used by proxies to make insecure requests.
    pub insecure_client: reqwest::Client,
    /// The location of the dist dir.
    pub dist_dir: PathBuf,
    /// The public URL from which assets are being served.
    pub public_url: String,
    /// The channel to receive build_done notifications on.
    pub build_done_chan: broadcast::Sender<()>,
    /// Whether to disable autoreload
    pub no_autoreload: bool,
    /// Additional headers to add to responses.
    pub headers: HashMap<String, String>,
}

impl State {
    /// Construct a new instance.
    pub fn new(
        dist_dir: PathBuf,
        public_url: String,
        client: reqwest::Client,
        insecure_client: reqwest::Client,
        cfg: &RtcServe,
        build_done_chan: broadcast::Sender<()>,
    ) -> Self {
        Self {
            client,
            insecure_client,
            dist_dir,
            public_url,
            build_done_chan,
            no_autoreload: cfg.no_autoreload,
            headers: cfg.headers.clone(),
        }
    }
}

/// Build the Trunk router, this includes that static file server, the WebSocket server,
/// (for autoreload & HMR in the future), as well as any user-defined proxies.
fn router(state: Arc<State>, cfg: Arc<RtcServe>) -> Result<Router> {
    // Build static file server, middleware, error handler & WS route for reloads.
    let public_route = if state.public_url == "/" {
        &state.public_url
    } else {
        state
            .public_url
            .strip_suffix('/')
            .unwrap_or(&state.public_url)
    };

    let mut serve_dir = get_service(
        ServeDir::new(&state.dist_dir).fallback(ServeFile::new(&state.dist_dir.join(INDEX_HTML))),
    );
    for (key, value) in &state.headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .with_context(|| format!("invalid header {:?}", key))?;
        let value: HeaderValue = value
            .parse()
            .with_context(|| format!("invalid header value {:?} for header {}", value, name))?;
        serve_dir = serve_dir.layer(SetResponseHeaderLayer::overriding(name, value))
    }

    let mut router = Router::new()
        .fallback(
            Router::new().nest(
                public_route,
                get_service(serve_dir)
                    .handle_error(|error| async move {
                        tracing::error!(?error, "failed serving static file");
                        StatusCode::INTERNAL_SERVER_ERROR
                    })
                    .layer(TraceLayer::new_for_http()),
            ),
        )
        .route(
            "/_trunk/ws",
            get(
                |ws: WebSocketUpgrade, state: Extension<Arc<State>>| async move {
                    ws.on_upgrade(|socket| async move { handle_ws(socket, state.0).await })
                },
            ),
        )
        .layer(Extension(state.clone()));

    tracing::info!(
        "{} serving static assets at -> {}",
        SERVER,
        state.public_url.as_str()
    );

    // Build proxies.
    if let Some(backend) = &cfg.proxy_backend {
        if cfg.proxy_ws {
            let handler = ProxyHandlerWebSocket::new(backend.clone(), cfg.proxy_rewrite.clone());
            router = handler.clone().register(router);
            tracing::info!(
                "{} proxying websocket {} -> {}",
                SERVER,
                handler.path(),
                &backend
            );
        } else {
            let client = if cfg.proxy_insecure {
                state.insecure_client.clone()
            } else {
                state.client.clone()
            };

            let handler = ProxyHandlerHttp::new(client, backend.clone(), cfg.proxy_rewrite.clone());
            router = handler.clone().register(router);
            tracing::info!("{} proxying {} -> {}", SERVER, handler.path(), &backend);
        }
    } else if let Some(proxies) = &cfg.proxies {
        for proxy in proxies.iter() {
            if proxy.ws {
                let handler =
                    ProxyHandlerWebSocket::new(proxy.backend.clone(), proxy.rewrite.clone());
                router = handler.clone().register(router);
                tracing::info!(
                    "{} proxying websocket {} -> {}",
                    SERVER,
                    handler.path(),
                    &proxy.backend
                );
            } else {
                let client = if proxy.insecure {
                    state.insecure_client.clone()
                } else {
                    state.client.clone()
                };

                let handler =
                    ProxyHandlerHttp::new(client, proxy.backend.clone(), proxy.rewrite.clone());
                router = handler.clone().register(router);
                tracing::info!(
                    "{} proxying {} -> {}",
                    SERVER,
                    handler.path(),
                    &proxy.backend
                );
            };
        }
    }

    Ok(router)
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
        let ws_send = ws.send(axum::extract::ws::Message::Text(
            r#"{"reload": true}"#.to_owned(),
        ));
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
    fn into_response(self) -> Response {
        tracing::error!(error = ?self.0, "error handling request");
        let mut res = Response::new(body::boxed(Body::empty()));
        *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        res
    }
}
