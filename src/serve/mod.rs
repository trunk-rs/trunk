mod proxy;

use crate::common::{nonce, LOCAL, NETWORK, SERVER};
use crate::config::rt::RtcServe;
use crate::tls::TlsConfig;
use crate::watch::WatchSystem;
use crate::ws;
use anyhow::{Context, Result};
use axum::body::{Body, Bytes};
use axum::extract;
use axum::extract::ws::WebSocketUpgrade;
use axum::http::header::{HeaderName, CONTENT_LENGTH, CONTENT_TYPE, HOST};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, get_service, Router};
use axum_server::Handle;
use futures_util::FutureExt;
use hickory_resolver::TokioAsyncResolver;
use http::header::CONTENT_SECURITY_POLICY;
use http::HeaderMap;
use proxy::{ProxyBuilder, ProxyClientOptions};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use tracing::log;

const INDEX_HTML: &str = "index.html";

/// A system encapsulating a build & watch system, responsible for serving generated content.
pub struct ServeSystem {
    cfg: Arc<RtcServe>,
    watch: WatchSystem,
    /// The URL to open when starting
    open_http_addr: String,
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
        let address = cfg.addresses.first().map_or_else(
            || SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), cfg.port),
            |ipaddr| SocketAddr::new(*ipaddr, cfg.port),
        );
        let base = cfg.serve_base()?;
        let open_http_addr = format!("{prefix}://{address}{base}");
        Ok(Self {
            cfg,
            watch,
            open_http_addr,
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
            if let Err(err) = open::that(self.open_http_addr) {
                tracing::error!(error = ?err, "error opening browser");
            }
        }
        drop(self.shutdown_tx); // Drop the broadcast channel to ensure it does not keep the system alive.

        select! {
            r = watch_handle => {
                match r {
                    Err(err) => {
                        tracing::error!(error = ?err, "error joining watch system handle");
                        Err(err)
                    }
                    _ => r,
                }?;
            },
            r = server_handle => {
                match r {
                    Err(err) => {
                        tracing::error!(error = ?err, "error joining server handle");
                        Err(err)
                    }
                    _ => r,
                }??;
            },
        }

        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(cfg, shutdown_rx))]
    async fn spawn_server(
        cfg: Arc<RtcServe>,
        shutdown_rx: broadcast::Receiver<()>,
        ws_state: watch::Receiver<ws::State>,
    ) -> Result<JoinHandle<Result<()>>> {
        let serve_base_url = cfg.serve_base()?;

        // Build the server.
        let state = Arc::new(State::new(
            cfg.watch.build.final_dist.clone(),
            serve_base_url.to_string(),
            cfg.clone(),
            ws_state,
        )?);
        let router = router(state, cfg.clone())?;

        let addr = cfg
            .addresses
            .iter()
            .map(|addr| (*addr, cfg.port).into())
            .collect::<Vec<_>>();

        let aliases = cfg
            .aliases
            .iter()
            .map(|alias| format!("{alias}:{}", cfg.port))
            .collect::<Vec<_>>();

        show_listening(
            &cfg,
            &addr,
            &aliases,
            &serve_base_url,
            !cfg.disable_address_lookup,
        )
        .await;

        let server = run_server(addr, cfg.tls.clone(), router, shutdown_rx);

        Ok(tokio::spawn(async move {
            match server.await {
                Err(err) => {
                    tracing::error!(error = ?err, "error from server task");
                    Err(err)
                }
                r => r,
            }
        }))
    }
}

/// Show where `serve` is listening
///
/// We'll look up addresses, and simply append aliases.
async fn show_listening(
    cfg: &RtcServe,
    addr: &[SocketAddr],
    aliases: &[String],
    base: &str,
    lookup: bool,
) {
    // Only show what we didn't show so far
    let mut cache = HashSet::new();

    let prefix = if cfg.tls.is_some() { "https" } else { "http" };

    // prepare interface addresses
    let interfaces = local_ip_address::list_afinet_netifas()
        .map(|addr| {
            addr.into_iter()
                .map(|(_name, addr)| addr)
                .collect::<Vec<_>>()
        })
        .unwrap_or(vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]);

    // prepare result
    let mut addresses = BTreeSet::<SocketAddr>::new();

    for addr in addr {
        if addr.ip().is_unspecified() {
            // it the "unspecified" address, so we add the corresponding address family addresses
            addresses.extend(interfaces.iter().filter_map(|ipaddr| match ipaddr {
                IpAddr::V4(_ip) if addr.is_ipv4() => Some(SocketAddr::new(*ipaddr, addr.port())),
                IpAddr::V6(_ip) if addr.is_ipv6() => Some(SocketAddr::new(*ipaddr, addr.port())),
                _ => None,
            }));
        } else {
            addresses.insert(*addr);
        }
    }

    fn is_loopback(address: &SocketAddr) -> bool {
        match address {
            SocketAddr::V4(addr) => addr.ip().is_loopback(),
            SocketAddr::V6(addr) => addr.ip().is_loopback(),
        }
    }

    tracing::info!("{SERVER}server listening at:");

    for address in &addresses {
        show_address(
            &mut cache,
            is_loopback(address),
            format!("{prefix}://{address}{base}"),
        );
    }
    for alias in aliases {
        show_address(&mut cache, true, alias);
    }
    if lookup {
        match TokioAsyncResolver::tokio_from_system_conf() {
            Ok(resolver) => {
                for address in &addresses {
                    let local = is_loopback(address);
                    if let Ok(names) = resolver.reverse_lookup(address.ip()).await {
                        for name in names {
                            show_address(
                                &mut cache,
                                local,
                                format!("{prefix}://{name}:{port}{base}", port = address.port()),
                            );
                        }
                    }
                }
            }
            Err(err) => {
                log::warn!("Failed to create system resolver, skipping address resolution: {err}");
            }
        }
    }
}

fn show_address(cache: &mut HashSet<String>, local: bool, address: impl Into<String>) {
    let address = address.into();
    if cache.insert(address.clone()) {
        tracing::info!("    {}{address}", if local { LOCAL } else { NETWORK });
    }
}

async fn run_server(
    addr: Vec<SocketAddr>,
    tls: Option<TlsConfig>,
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
            Some(tls) =>
            {
                #[allow(unreachable_code)]
                match tls.clone() {
                    #[cfg(feature = "rustls")]
                    TlsConfig::Rustls { config } => {
                        tasks.push(
                            async move {
                                axum_server::bind_rustls(addr, config)
                                    .handle(shutdown_handle)
                                    .serve(router.into_make_service())
                                    .await
                            }
                            .boxed(),
                        );
                    }
                    #[cfg(feature = "native-tls")]
                    TlsConfig::Native { config } => {
                        tasks.push(
                            async move {
                                axum_server::bind_openssl(addr, config)
                                    .handle(shutdown_handle)
                                    .serve(router.into_make_service())
                                    .await
                            }
                            .boxed(),
                        );
                    }
                }
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

    let (result, _, _) = futures_util::future::select_all(tasks).await;
    Ok(result?)
}

/// Server state.
pub struct State {
    /// The location of the dist dir.
    pub dist_dir: PathBuf,
    /// The public URL from which assets are being served.
    pub serve_base: String,
    /// The channel for WS client messages.
    pub ws_state: watch::Receiver<ws::State>,
    /// The path to the autoreload websocket
    pub ws_base: String,
    /// Additional headers to add to responses.
    pub headers: HashMap<String, String>,
    /// Configuration
    pub cfg: Arc<RtcServe>,
}

impl State {
    /// Construct a new instance.
    pub fn new(
        dist_dir: PathBuf,
        serve_base: String,
        cfg: Arc<RtcServe>,
        ws_state: watch::Receiver<ws::State>,
    ) -> Result<Self> {
        let mut ws_base = cfg.ws_base()?.to_string();
        if !ws_base.ends_with('/') {
            ws_base.push('/');
        }

        Ok(Self {
            dist_dir,
            serve_base,
            ws_state,
            ws_base,
            headers: cfg.headers.clone(),
            cfg,
        })
    }
}

/// Build the Trunk router, this includes that static file server, the WebSocket server,
/// (for autoreload & HMR in the future), as well as any user-defined proxies.
fn router(state: Arc<State>, cfg: Arc<RtcServe>) -> Result<Router> {
    // Build static file server, middleware, error handler & WS route for reloads.

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

    let mut router = Router::new()
        .route(
            // we always serve the ws under the serve-base, ws-base is only to override the lookup
            "/.well-known/trunk/ws",
            get(
                |ws: WebSocketUpgrade, state: axum::extract::State<Arc<State>>| async move {
                    ws.on_upgrade(|socket| async move { ws::handle_ws(socket, state.0).await })
                },
            ),
        )
        .fallback_service(
            get_service(serve_dir)
                .handle_error(|error| async move {
                    tracing::error!(?error, "failed serving static file");
                    StatusCode::INTERNAL_SERVER_ERROR
                })
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    html_address_middleware,
                )),
        )
        .layer(TraceLayer::new_for_http());

    if state.serve_base != "/" {
        router = Router::new().nest(&state.serve_base, router);
    }

    let router = router.with_state(state.clone());

    tracing::info!(
        "{}serving static assets at -> {}",
        SERVER,
        state.serve_base.as_str()
    );

    let mut builder = ProxyBuilder::new(cfg.tls.is_some(), router);

    // Build proxies

    for proxy in &cfg.proxies {
        let mut request_headers = HeaderMap::new();
        for (key, value) in &proxy.request_headers {
            let name = HeaderName::from_bytes(key.as_bytes())
                .with_context(|| format!("invalid header {:?}", key))?;
            let value: HeaderValue = value
                .parse()
                .with_context(|| format!("invalid header value {:?} for header {}", value, name))?;
            request_headers.insert(name, value);
        }

        builder = builder.register_proxy(
            proxy.ws,
            &proxy.backend,
            &request_headers,
            proxy.rewrite.clone(),
            ProxyClientOptions {
                insecure: proxy.insecure,
                no_system_proxy: proxy.no_system_proxy,
                redirect: !proxy.no_redirect,
            },
        )?;
    }

    Ok(builder.build())
}

async fn html_address_middleware(
    extract::State(state): extract::State<Arc<State>>,
    request: extract::Request,
    next: Next,
) -> Response {
    let host = request.headers().get(HOST).cloned();

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

    let nonce = state
        .cfg
        .create_nonce
        .as_ref()
        .map(|p| (p.as_str(), nonce()));

    // turn the body into bytes
    match axum::body::to_bytes(body, 100 * 1024 * 1024).await {
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
                    let host = host
                        .and_then(|uri| uri.to_str().map(|s| format!("'{}'", s)).ok())
                        .unwrap_or_else(|| "window.location.host".into());

                    let mut data_str = data_str
                        // minification will turn quotes into backticks, so we have to replace both
                        .replace("'{{__TRUNK_ADDRESS__}}'", &host)
                        .replace("`{{__TRUNK_ADDRESS__}}`", &host)
                        // here we only replace the string value
                        .replace("{{__TRUNK_WS_BASE__}}", &state.ws_base);

                    let mut csp = None;

                    if let Some((var, val)) = nonce {
                        data_str = data_str.replace(var, &val);
                        csp = state
                            .cfg
                            .csp
                            .as_ref()
                            .map(|csp| csp.join(";").replace("{{NONCE}}", &val).parse());
                    }

                    match csp {
                        Some(Ok(csp)) => {
                            parts.headers.insert(CONTENT_SECURITY_POLICY, csp);
                        }
                        Some(Err(e)) => tracing::error!("failed to encode csp header: {e:?}"),
                        None => {}
                    };

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

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        tracing::error!(error = ?self.0, "error handling request");
        let mut res = Response::new(Body::empty());
        *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        res
    }
}
