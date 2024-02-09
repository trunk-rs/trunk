use crate::common::{LOCAL, NETWORK, SERVER};
use crate::config::RtcServe;
use crate::proxy::{ProxyHandlerHttp, ProxyHandlerWebSocket};
use crate::watch::WatchSystem;
use crate::ws;
use anyhow::{Context, Result};
use axum::body::{self, Body, Bytes};
use axum::extract::ws::WebSocketUpgrade;
use axum::http::header::{HeaderName, CONTENT_LENGTH, CONTENT_TYPE, HOST};
use axum::http::{HeaderValue, Request, StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, get_service, Router};
use axum_server::tls_rustls::RustlsConfig;
use axum_server::Handle;
use futures_util::FutureExt;
use reqwest::Client;
use std::collections::{hash_map::Entry, BTreeSet, HashMap};
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
        let address = match cfg.addresses.first() {
            Some(address) => *address,
            None => IpAddr::V4(Ipv4Addr::LOCALHOST),
        };
        let http_addr = format!(
            "{}://{}:{}{}",
            prefix, address, cfg.port, &cfg.watch.build.public_url
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
        // Build the server.
        let state = Arc::new(State::new(
            cfg.watch.build.final_dist.clone(),
            cfg.watch.build.public_url.clone(),
            &cfg,
            ws_state,
        ));
        let router = router(state, cfg.clone())?;

        let addr = cfg
            .addresses
            .iter()
            .map(|addr| (*addr, cfg.port).into())
            .collect::<Vec<_>>();

        let server = run_server(addr.clone(), cfg.tls.clone(), router, shutdown_rx);

        show_listening(&cfg, &addr);

        // Block this routine on the server's completion.
        Ok(tokio::spawn(async move {
            if let Err(err) = server.await {
                tracing::error!(error = ?err, "error from server task");
            }
        }))
    }
}

/// show where `serve` is listening
fn show_listening(cfg: &RtcServe, addr: &[SocketAddr]) {
    let prefix = if cfg.tls.is_some() { "https" } else { "http" };

    // prepare local addresses
    let locals = local_ip_address::list_afinet_netifas()
        .map(|addr| {
            addr.into_iter()
                .map(|(_name, addr)| addr)
                .filter(|addr| addr.is_loopback())
                .collect::<Vec<_>>()
        })
        .unwrap_or(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);

    // prepare result
    let mut addresses = BTreeSet::<SocketAddr>::new();

    for addr in addr {
        if addr.ip().is_unspecified() {
            addresses.extend(locals.iter().filter_map(|ipaddr| match ipaddr {
                IpAddr::V4(_ip) if addr.is_ipv4() => Some(SocketAddr::new(*ipaddr, addr.port())),
                IpAddr::V6(_ip) if addr.is_ipv6() => Some(SocketAddr::new(*ipaddr, addr.port())),
                _ => None,
            }));
        } else {
            addresses.insert(*addr);
        }
    }

    fn is_loopback(address: SocketAddr) -> bool {
        match address {
            SocketAddr::V4(addr) => addr.ip().is_loopback(),
            SocketAddr::V6(addr) => addr.ip().is_loopback(),
        }
    }

    tracing::info!("{SERVER}server listening at:");

    for address in addresses {
        tracing::info!(
            "    {}{}://{}",
            if is_loopback(address) { LOCAL } else { NETWORK },
            prefix,
            address,
        );
    }
}

async fn run_server(
    addr: Vec<SocketAddr>,
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

    let mut tasks = vec![];

    for addr in addr {
        let router = router.clone();
        let shutdown_handle = shutdown_handle.clone();
        match &tls {
            Some(tls_config) => {
                tasks.push(
                    async move {
                        axum_server::bind_rustls(addr, tls_config.clone())
                            .handle(shutdown_handle)
                            .serve(router.into_make_service())
                            .await
                    }
                    .boxed(),
                );
            }
            None => tasks.push(
                async move {
                    axum_server::bind(addr)
                        .handle(shutdown_handle)
                        .serve(router.into_make_service())
                        .await
                }
                .boxed(),
            ),
        };
    }

    futures_util::future::join_all(tasks).await;

    Ok(())
}

/// Server state.
pub struct State {
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
        cfg: &RtcServe,
        ws_state: watch::Receiver<ws::State>,
    ) -> Self {
        Self {
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
        get_service(
            ServeDir::new(&state.dist_dir)
                .fallback(ServeFile::new(state.dist_dir.join(INDEX_HTML))),
        )
    };
    for (key, value) in &state.headers {
        let name = HeaderName::from_bytes(key.as_bytes())
            .with_context(|| format!("invalid header {:?}", key))?;
        let value: HeaderValue = value
            .parse()
            .with_context(|| format!("invalid header value {:?} for header {}", value, name))?;
        serve_dir = serve_dir.layer(SetResponseHeaderLayer::overriding(name, value))
    }

    let router = Router::new()
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
        "{}serving static assets at -> {}",
        SERVER,
        state.public_url.as_str()
    );

    let mut builder = ProxyBuilder::new(router);

    // Build proxies.
    if let Some(backend) = &cfg.proxy_backend {
        builder = builder.register_proxy(
            cfg.proxy_ws,
            backend,
            cfg.proxy_rewrite.clone(),
            ProxyClientOptions {
                insecure: cfg.proxy_insecure,
                no_system_proxy: cfg.proxy_no_sys_proxy,
            },
        )?;
    } else if let Some(proxies) = &cfg.proxies {
        for proxy in proxies.iter() {
            builder = builder.register_proxy(
                proxy.ws,
                &proxy.backend,
                proxy.rewrite.clone(),
                ProxyClientOptions {
                    insecure: proxy.insecure,
                    no_system_proxy: proxy.no_sys_proxy,
                },
            )?;
        }
    }

    Ok(builder.build())
}

/// A builder for the proxy router
pub(crate) struct ProxyBuilder {
    router: Router,
    clients: ProxyClients,
}

impl ProxyBuilder {
    /// Create a new builder
    pub fn new(router: Router) -> Self {
        Self {
            router,
            clients: Default::default(),
        }
    }

    /// Register a new proxy config
    pub fn register_proxy(
        mut self,
        ws: bool,
        backend: &Uri,
        rewrite: Option<String>,
        opts: ProxyClientOptions,
    ) -> Result<Self> {
        if ws {
            let handler = ProxyHandlerWebSocket::new(backend.clone(), rewrite);
            tracing::info!(
                "{}proxying websocket {} -> {}",
                SERVER,
                handler.path(),
                &backend
            );
            self.router = handler.register(self.router);
            Ok(self)
        } else {
            let no_sys_proxy = opts.no_system_proxy;
            let insecure = opts.insecure;
            let client = self.clients.get_client(opts)?;
            let handler = ProxyHandlerHttp::new(client, backend.clone(), rewrite);
            tracing::info!(
                "{}proxying {} -> {}{}{}",
                SERVER,
                handler.path(),
                &backend,
                if no_sys_proxy {
                    "; ignoring system proxy"
                } else {
                    ""
                },
                if insecure {
                    "; ⚠️ insecure TLS"
                } else {
                    ""
                }
            );
            self.router = handler.register(self.router);
            Ok(self)
        }
    }

    pub fn build(self) -> Router {
        self.router
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
pub(crate) struct ProxyClientOptions {
    pub insecure: bool,
    pub no_system_proxy: bool,
}

#[derive(Default)]
pub(crate) struct ProxyClients {
    clients: HashMap<ProxyClientOptions, Client>,
}

impl ProxyClients {
    pub fn get_client(&mut self, opts: ProxyClientOptions) -> Result<Client> {
        match self.clients.entry(opts.clone()) {
            Entry::Occupied(entry) => Ok(entry.get().clone()),
            Entry::Vacant(entry) => {
                let client = Self::create_client(opts)?;
                entry.insert(client.clone());
                Ok(client)
            }
        }
    }

    /// Create a new client for proxying
    fn create_client(opts: ProxyClientOptions) -> Result<Client> {
        let mut builder = reqwest::ClientBuilder::new().http1_only();
        if opts.insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        if opts.no_system_proxy {
            builder = builder.no_proxy();
        }
        builder.build().context("error building proxy client")
    }
}

async fn html_address_middleware<B: std::fmt::Debug>(
    request: Request<B>,
    next: Next<B>,
) -> Response {
    let uri = request.headers().get(HOST).cloned();
    let response = next.run(request).await;

    // if it's not a success, we don't modify it
    if !response.status().is_success() {
        return response;
    }

    // if it doesn't look like HTML, we ignore it too
    let is_html = response
        .headers()
        .get(CONTENT_TYPE)
        .map(|t| t == "text/html")
        .unwrap_or_default();
    if !is_html {
        return response;
    }

    // split into parts and body
    let (parts, body) = response.into_parts();

    // turn the body into bytes
    match hyper::body::to_bytes(body).await {
        Err(err) => {
            tracing::debug!("Unable to intercept: {err}");
            (parts, Bytes::default()).into_response()
        }
        Ok(bytes) => {
            let mut parts = parts;
            let mut bytes = bytes;

            match std::str::from_utf8(&bytes) {
                Ok(data_str) => {
                    tracing::debug!("Replacing variable");

                    // turn into a string literal, or replace with "current host" on the client side
                    let uri = uri
                        .and_then(|uri| uri.to_str().map(|s| format!("'{}'", s)).ok())
                        .unwrap_or_else(|| "window.location.host".into());

                    let data_str = data_str
                        .replace("'{{__TRUNK_ADDRESS__}}'", &uri)
                        // minification will turn that into backticks
                        .replace("`{{__TRUNK_ADDRESS__}}`", &uri);
                    let bytes_vec = data_str.as_bytes().to_vec();
                    parts.headers.insert(CONTENT_LENGTH, bytes_vec.len().into());
                    bytes = Bytes::from(bytes_vec);
                }
                Err(err) => {
                    tracing::debug!("Unable to parse for injecting: {err}");
                }
            }

            (parts, bytes).into_response()
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
