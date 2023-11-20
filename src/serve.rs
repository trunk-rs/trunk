use crate::common::{LOCAL, NETWORK, SERVER};
use crate::config::RtcServe;
use crate::proxy::{ProxyHandlerHttp, ProxyHandlerWebSocket};
use crate::watch::WatchSystem;
use crate::ws;
use anyhow::{Context, Result};
use axum::body::{self, Body, Bytes};
use axum::extract::ws::WebSocketUpgrade;
use axum::http::header::{HeaderName, CONTENT_LENGTH, CONTENT_TYPE, HOST};
use axum::http::response::Parts;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::routing::{get, get_service, Router};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

const INDEX_HTML: &str = "index.html";

/// A system encapsulating a build & watch system, responsible for serving generated content.
pub struct ServeSystem {
    cfg: Arc<RtcServe>,
    watch: WatchSystem,
    http_addr: String,
    shutdown_tx: broadcast::Sender<()>,
    //  N.B. we use a broadcast channel here because a watch channel triggers a
    //  false positive on the first read of channel
    ws_state: watch::Receiver<ws::State>,
}

impl ServeSystem {
    /// Construct a new instance.
    pub async fn new(cfg: Arc<RtcServe>, shutdown: broadcast::Sender<()>) -> Result<Self> {
        let (ws_state_tx, ws_state) = watch::channel(ws::State::default());
        let watch = WatchSystem::new(
            cfg.watch.clone(),
            shutdown.clone(),
            Some(ws_state_tx),
            cfg.ws_protocol,
        )
        .await?;
        let prefix = if cfg.tls.is_some() { "https" } else { "http" };
        let http_addr = format!(
            "{}://{}:{}{}",
            prefix, cfg.address, cfg.port, &cfg.watch.build.public_url
        );
        Ok(Self {
            cfg,
            watch,
            http_addr,
            shutdown_tx: shutdown,
            ws_state,
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
            self.ws_state,
        )
        .await?;

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
    async fn spawn_server(
        cfg: Arc<RtcServe>,
        shutdown_rx: broadcast::Receiver<()>,
        ws_state: watch::Receiver<ws::State>,
    ) -> Result<JoinHandle<()>> {
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
            ws_state,
        ));
        let router = router(state, cfg.clone())?;
        let addr = (cfg.address, cfg.port).into();

        let server = run_server(addr, cfg.tls.clone(), router, shutdown_rx);

        let prefix = if cfg.tls.is_some() { "https" } else { "http" };
        if addr.ip().is_unspecified() {
            let addresses = local_ip_address::list_afinet_netifas()
                .map(|addrs| {
                    addrs
                        .into_iter()
                        .filter_map(|(_, ipaddr)| match ipaddr {
                            IpAddr::V4(ip) if ip.is_private() || ip.is_loopback() => Some(ip),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|_| vec![Ipv4Addr::LOCALHOST]);
            tracing::info!(
                "{} server listening at:\n{}",
                SERVER,
                addresses
                    .iter()
                    .map(|address| format!(
                        "    {} {}://{}:{}",
                        if address.is_loopback() {
                            LOCAL
                        } else {
                            NETWORK
                        },
                        prefix,
                        address,
                        cfg.port
                    ))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        } else {
            tracing::info!("{} server listening at {}://{}", SERVER, prefix, addr);
        }
        // Block this routine on the server's completion.
        Ok(tokio::spawn(async move {
            if let Err(err) = server.await {
                tracing::error!(error = ?err, "error from server task");
            }
        }))
    }
}

async fn run_server(
    addr: SocketAddr,
    tls: Option<RustlsConfig>,
    router: Router,
    mut shutdown_rx: broadcast::Receiver<()>,
) -> Result<()> {
    // Build a shutdown signal for the axum server.
    let shutdown_handle = Handle::new();

    let shutdown = |handle: Handle| async move {
        // Any event on this channel, even a drop, should trigger shutdown.
        let _res = shutdown_rx.recv().await;
        tracing::debug!("server is shutting down");
        handle.graceful_shutdown(Some(Duration::from_secs(0)));
    };

    tokio::spawn(shutdown(shutdown_handle.clone()));
    match tls {
        Some(tls_config) => {
            axum_server::bind_rustls(addr, tls_config.clone())
                .handle(shutdown_handle)
                .serve(router.into_make_service())
                .await
        }
        None => {
            axum_server::bind(addr)
                .handle(shutdown_handle)
                .serve(router.into_make_service())
                .await
        }
    }?;

    Ok(())
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
    /// The channel for WS client messages.
    pub ws_state: watch::Receiver<ws::State>,
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
        ws_state: watch::Receiver<ws::State>,
    ) -> Self {
        Self {
            client,
            insecure_client,
            dist_dir,
            public_url,
            ws_state,
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

    let mut serve_dir = if cfg.no_spa {
        get_service(ServeDir::new(&state.dist_dir))
    } else {
        get_service(ServeDir::new(&state.dist_dir).fallback(ServeFile::new(state.dist_dir.join(INDEX_HTML))))
    };
    for (key, value) in &state.headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .with_context(|| format!("invalid header {:?}", key))?;
        let value: HeaderValue = value
            .parse()
            .with_context(|| format!("invalid header value {:?} for header {}", value, name))?;
        serve_dir = serve_dir.layer(SetResponseHeaderLayer::overriding(name, value))
    }

    let mut router = Router::new()
        .fallback_service(
            Router::new().nest_service(
                public_route,
                get_service(serve_dir)
                    .handle_error(|error| async move {
                        tracing::error!(?error, "failed serving static file");
                        StatusCode::INTERNAL_SERVER_ERROR
                    })
                    .layer(TraceLayer::new_for_http())
                    .layer(axum::middleware::from_fn(html_address_middleware)),
            ),
        )
        .route(
            "/_trunk/ws",
            get(
                |ws: WebSocketUpgrade, state: axum::extract::State<Arc<State>>| async move {
                    ws.on_upgrade(|socket| async move { ws::handle_ws(socket, state.0).await })
                },
            ),
        )
        .with_state(state.clone());

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

async fn html_address_middleware<B: std::fmt::Debug>(
    request: Request<B>,
    next: Next<B>,
) -> (Parts, Bytes) {
    let uri = request.headers().get(HOST).cloned();
    let response = next.run(request).await;
    let (parts, body) = response.into_parts();

    match hyper::body::to_bytes(body).await {
        Err(_) => (parts, Bytes::default()),
        Ok(bytes) => {
            let (mut parts, mut bytes) = (parts, bytes);

            if let Some(uri) = uri {
                if parts
                    .headers
                    .get(CONTENT_TYPE)
                    .map(|t| t == "text/html")
                    .unwrap_or(false)
                {
                    if let Ok(data_str) = std::str::from_utf8(&bytes) {
                        let data_str = data_str.replace(
                            "'{{__TRUNK_ADDRESS__}}'",
                            &uri.to_str()
                                .map(|s| format!("'{}'", s))
                                .unwrap_or_else(|_| "window.location.href".into()),
                        );
                        let bytes_vec = data_str.as_bytes().to_vec();
                        parts.headers.insert(CONTENT_LENGTH, bytes_vec.len().into());
                        bytes = Bytes::from(bytes_vec);
                    }
                }
            }

            (parts, bytes)
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
