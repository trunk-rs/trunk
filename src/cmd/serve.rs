use crate::{
    config::{
        self,
        models::Proxy,
        rt::{self, RtcBuilder, RtcServe},
        types::{AddressFamily, WsProtocol},
        Configuration,
    },
    serve::ServeSystem,
};
use anyhow::{Context, Result};
use axum::http::Uri;
use clap::Args;
use std::{net::IpAddr, path::PathBuf, sync::Arc};
use tokio::{select, sync::broadcast};

/// Build, watch & serve the Rust WASM app and all of its assets.
#[derive(Clone, Args)]
#[command(name = "serve")]
#[command(next_help_heading = "Serve")]
pub struct Serve {
    /// The addresses to serve on [default: <local loopback>]
    #[arg(short, long, env = "TRUNK_SERVE_ADDRESS")]
    pub address: Option<Vec<IpAddr>>,
    #[arg(short = 'A', long, env = "TRUNK_SERVE_PREFER_ADDRESS_FAMILY")]
    pub prefer_address_family: Option<AddressFamily>,
    /// The port to serve on [default: 8080]
    #[arg(short, long, env = "TRUNK_SERVE_PORT")]
    pub port: Option<u16>,
    /// The aliases to serve on
    #[arg(long, env = "TRUNK_SERVE_ALIAS")]
    pub alias: Option<Vec<String>>,
    /// Disable the lookup of addresses serving on during startup
    #[arg(long, env = "TRUNK_SERVE_DISABLE_ADDRESS_LOOKUP")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub disable_address_lookup: Option<bool>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[arg(long, env = "TRUNK_SERVE_OPEN")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub open: Option<bool>,
    /// Disable auto-reload of the web app
    #[arg(long, env = "TRUNK_SERVE_NO_AUTORELOAD")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub no_autoreload: Option<bool>,
    /// Disable error reporting in the browser [default: false]
    #[arg(long, env = "TRUNK_SERVE_NO_ERROR_REPORTING")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub no_error_reporting: Option<bool>,
    /// Disable fallback to index.html for missing files [default: false]
    #[arg(long, env = "TRUNK_SERVE_NO_SPA")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub no_spa: Option<bool>,
    /// Protocol used for the auto-reload WebSockets connection [enum: ws, wss]
    #[arg(long, env = "TRUNK_SERVE_WS_PROTOCOL")]
    pub ws_protocol: Option<WsProtocol>,
    /// The path to the trunk web-socket [default: <serve-base>]
    #[arg(long, env = "TRUNK_SERVE_WS_BASE")]
    pub ws_base: Option<String>,
    /// The TLS key file to enable TLS encryption [default: None]
    #[arg(long, env = "TRUNK_SERVE_TLS_KEY_PATH")]
    pub tls_key_path: Option<PathBuf>,
    /// The TLS cert file to enable TLS encryption [default: None]
    #[arg(long, env = "TRUNK_SERVE_TLS_CERT_PATH")]
    pub tls_cert_path: Option<PathBuf>,
    /// A base path to serve the application from [default: <public-url>]
    #[arg(long, env = "TRUNK_SERVE_SERVE_BASE")]
    pub serve_base: Option<String>,
    /// Disable Content-Security-Policy [default: false]
    #[arg(long)]
    #[arg(default_missing_value="false", num_args=0..=1)]
    pub disable_csp: Option<bool>,

    // NOTE: flattened structures come last
    #[command(flatten)]
    pub proxy: ProxyArgs,

    #[command(flatten)]
    pub watch: super::watch::Watch,
}

#[derive(Clone, Debug, Default, Args)]
#[command(next_help_heading = "Backend Proxy")]
pub struct ProxyArgs {
    /// A URL to which requests will be proxied
    #[arg(long, env = "TRUNK_SERVE_PROXY_BACKEND")]
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[arg(long, env = "TRUNK_SERVE_PROXY_REWRITE", requires = "proxy_backend")]
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets
    #[arg(long, env = "TRUNK_SERVE_PROXY_WS", requires = "proxy_backend")]
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure requests
    #[arg(long, env = "TRUNK_SERVE_PROXY_INSECURE", requires = "proxy_backend")]
    pub proxy_insecure: bool,
    /// Configure the proxy to bypass system proxy when contacting the backend
    #[arg(
        long,
        env = "TRUNK_SERVE_PROXY_NO_SYSTEM_PROXY",
        requires = "proxy_backend"
    )]
    pub proxy_no_system_proxy: bool,
    /// Configure the proxy to not automatically follow redirects if a backend responds with a redirect
    #[arg(
        long,
        env = "TRUNK_SERVE_PROXY_NO_REDIRECT",
        requires = "proxy_backend"
    )]
    pub proxy_no_redirect: bool,
}

impl Serve {
    /// apply CLI overrides to the configuration
    fn apply_to(self, mut config: Configuration) -> Result<Configuration> {
        let Self {
            address,
            prefer_address_family,
            port,
            alias,
            disable_address_lookup,
            open,
            proxy:
                ProxyArgs {
                    proxy_backend,
                    proxy_rewrite,
                    proxy_ws,
                    proxy_insecure,
                    proxy_no_system_proxy,
                    proxy_no_redirect,
                },
            no_autoreload,
            no_error_reporting,
            no_spa,
            ws_protocol,
            ws_base,
            tls_key_path,
            tls_cert_path,
            serve_base,
            watch,
            disable_csp,
        } = self;

        // apply overrides

        config.serve.addresses = address.unwrap_or(config.serve.addresses);
        config.serve.port = port.unwrap_or(config.serve.port);
        config.serve.aliases = alias.unwrap_or(config.serve.aliases);
        config.serve.disable_address_lookup =
            disable_address_lookup.unwrap_or(config.serve.disable_address_lookup);
        config.serve.open = open.unwrap_or(config.serve.open);
        config.serve.prefer_address_family =
            prefer_address_family.or(config.serve.prefer_address_family);
        config.serve.serve_base = serve_base.or(config.serve.serve_base);

        config.serve.tls_key_path = tls_key_path.or(config.serve.tls_key_path);
        config.serve.tls_cert_path = tls_cert_path.or(config.serve.tls_cert_path);

        config.serve.no_autoreload = no_autoreload.unwrap_or(config.serve.no_autoreload);
        config.serve.no_error_reporting =
            no_error_reporting.unwrap_or(config.serve.no_error_reporting);
        config.serve.no_spa = no_spa.unwrap_or(config.serve.no_spa);

        config.serve.ws_protocol = ws_protocol.or(config.serve.ws_protocol);
        config.serve.ws_base = ws_base.or(config.serve.ws_base);
        config.serve.disable_csp = disable_csp.unwrap_or(config.serve.disable_csp);

        if let Some(backend) = proxy_backend {
            // we have a single proxy from the command line
            config.proxies.0.push(Proxy {
                backend: backend.into(),
                request_headers: Default::default(),
                rewrite: proxy_rewrite,
                ws: proxy_ws,
                insecure: proxy_insecure,
                no_system_proxy: proxy_no_system_proxy,
                no_redirect: proxy_no_redirect,
            });
        }

        // apply base layer

        let config = watch.apply_to(config)?;

        // done

        Ok(config)
    }

    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (cfg, working_directory) = config::load(config).await?;

        let cfg = self.clone().apply_to(cfg)?;
        let cfg = RtcServe::from_config(cfg, working_directory, |cfg, core| rt::ServeOptions {
            watch: rt::WatchOptions {
                build: rt::BuildOptions {
                    core,
                    inject_autoloader: !cfg.serve.no_autoreload,
                },
                poll: self.watch.poll.then_some(self.watch.poll_interval.0),
                enable_cooldown: self.watch.enable_cooldown,
                clear_screen: self.watch.clear_screen,
                no_error_reporting: cfg.serve.no_error_reporting,
            },
            // This will be the effective value for `serve.open` during runtime.
            open: self.open.unwrap_or(cfg.serve.open),
        })
        .await?;

        cfg.enforce_version()?;

        let (shutdown_tx, _) = broadcast::channel(1);

        let system = ServeSystem::new(Arc::new(cfg), shutdown_tx.clone()).await?;

        let system_handle = tokio::spawn(system.run());

        select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::debug!("received shutdown signal");
                shutdown_tx.send(()).ok();
                drop(shutdown_tx);
            }
            r = system_handle => {
                r.context("error awaiting system shutdown")??;
            }
        }

        tracing::debug!("Exiting serve main");

        Ok(())
    }
}
