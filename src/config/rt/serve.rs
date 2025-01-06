use crate::{
    config::{
        models::{Proxy, Serve},
        rt::{RtcBuilder, RtcWatch, WatchOptions},
        types::{AddressFamily, BaseUrl, WsProtocol},
        Configuration,
    },
    tls::TlsConfig,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use local_ip_address::list_afinet_netifas;
use std::{
    borrow::Cow,
    collections::HashMap,
    net::{IpAddr, Ipv4Addr},
    ops::Deref,
    path::PathBuf,
    sync::Arc,
};
use tracing::log;

/// Runtime config for the serve system.
#[derive(Clone, Debug)]
pub struct RtcServe {
    /// Runtime config for the watch system.
    pub watch: Arc<RtcWatch>,
    /// The IP address to serve on.
    pub addresses: Vec<IpAddr>,
    /// The port to serve on.
    pub port: u16,
    /// The aliases to serve on.
    pub aliases: Vec<String>,
    /// Disable the DNS lookup during startup
    pub disable_address_lookup: bool,
    /// Open a browser tab once the initial build is complete.
    pub open: bool,
    /// Any proxies configured to run along with the server.
    pub proxies: Vec<Proxy>,
    /// Whether to disable fallback to index.html for missing files.
    pub no_spa: bool,
    /// Additional headers to include in responses.
    pub headers: HashMap<String, String>,
    /// Protocol used for autoreload WebSockets connection.
    pub ws_protocol: Option<WsProtocol>,
    /// Path used for autoreload WebSockets connection.
    pub ws_base: Option<String>,
    /// The TLS config containing the certificate and private key. TLS is activated if both are set.
    pub tls: Option<TlsConfig>,
    /// A base path to serve the application from
    pub serve_base: Option<String>,
    /// Disable Content-Security-Policy
    pub csp: Option<Vec<String>>,
}

impl Deref for RtcServe {
    type Target = RtcWatch;

    fn deref(&self) -> &Self::Target {
        &self.watch
    }
}

#[derive(Clone, Debug)]
pub struct ServeOptions {
    pub watch: WatchOptions,
    pub open: bool,
}

impl RtcServe {
    /// Construct a new instance
    pub(crate) async fn new(config: Configuration, opts: ServeOptions) -> Result<Self> {
        let ServeOptions {
            watch: watch_opts,
            open,
        } = opts;

        let watch = Arc::new(RtcWatch::new(config.clone(), watch_opts)?);

        #[allow(deprecated)]
        let Serve {
            address: _,
            addresses,
            prefer_address_family,
            port,
            aliases,
            disable_address_lookup,
            open: _,
            // auto-reload is handle by the builder options
            no_autoreload: _,
            headers,
            no_error_reporting: _, // handled via the options, as it's only a configuration option in the case of "serve"
            no_spa,
            ws_protocol,
            ws_base,
            tls_key_path,
            tls_cert_path,
            serve_base,
            // single proxy config is being transformed into global proxies vec
            proxy_backend: _,
            proxy_rewrite: _,
            proxy_ws: _,
            proxy_insecure: _,
            proxy_no_system_proxy: _,
            proxy_no_redirect: _,
            disable_csp,
            csp,
        } = config.serve;

        let tls = tls_config(
            absolute_path_if_some(tls_key_path, "tls_key_path")?,
            absolute_path_if_some(tls_cert_path, "tls_cert_path")?,
        )
        .await?;

        Ok(Self {
            watch,
            addresses: build_address_list(prefer_address_family, addresses),
            port,
            aliases,
            disable_address_lookup,
            open,
            proxies: config.proxies.0,
            no_spa,
            headers,
            ws_protocol,
            ws_base,
            tls,
            serve_base,
            csp: (!disable_csp).then_some(csp),
        })
    }

    fn common_base(&self) -> Result<Cow<str>> {
        let base = match &self.watch.build.public_url {
            BaseUrl::Default => "/",
            BaseUrl::Absolute(url) => {
                tracing::warn!(
                    url = url.as_str(),
                    "Using the path component of an absolute URL for serving"
                );
                tracing::warn!(
                    "You can silence this warning by using an explicit serve-base value"
                );
                url.path()
            }
            BaseUrl::AbsolutePath(url) => url,
            BaseUrl::RelativePath(path) if path == "./" => "/",
            BaseUrl::RelativePath(path) => {
                tracing::warn!(
                    path,
                    "Using the relative path as an absolute path for serving"
                );
                tracing::warn!(
                    "You can silence this warning by using an explicit serve-base value"
                );
                if let Some(path) = path.strip_prefix('.') {
                    path
                } else {
                    return Ok(Cow::Owned(format!("/{path}")));
                }
            }
        };

        Ok(base.into())
    }

    pub(crate) fn ws_base(&self) -> Result<Cow<str>> {
        if let Some(ws_path) = &self.ws_base {
            ensure!(ws_path.starts_with('/'), "ws-path must start with a '/'");
            return Ok(ws_path.into());
        }

        self.common_base()
    }

    pub(crate) fn serve_base(&self) -> Result<Cow<str>> {
        if let Some(serve_base) = &self.serve_base {
            ensure!(
                serve_base.starts_with('/'),
                "serve-base must start with a '/'"
            );
            return Ok(serve_base.into());
        }

        self.common_base()
    }
}

impl RtcBuilder for RtcServe {
    type Options = ServeOptions;

    async fn build(configuration: Configuration, options: Self::Options) -> Result<Self> {
        Self::new(configuration, options).await
    }
}

fn build_address_list(preference: Option<AddressFamily>, addresses: Vec<IpAddr>) -> Vec<IpAddr> {
    if !addresses.is_empty() {
        addresses
    } else {
        match list_afinet_netifas() {
            Ok(ifas) => ifas
                .into_iter()
                .filter_map(
                    |(_name, addr)| {
                        if addr.is_loopback() {
                            Some(addr)
                        } else {
                            None
                        }
                    },
                )
                .filter(|addr| match preference {
                    None => true,
                    Some(AddressFamily::Ipv6) if addr.is_ipv6() => true,
                    Some(AddressFamily::Ipv4) if addr.is_ipv4() => true,
                    _ => false,
                })
                .collect(),
            Err(err) => {
                log::warn!("Unable to list network interfaces: {err}");
                vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]
            }
        }
    }
}

#[allow(unreachable_code)]
async fn tls_config(
    tls_key_path: Option<PathBuf>,
    tls_cert_path: Option<PathBuf>,
) -> Result<Option<TlsConfig>, anyhow::Error> {
    match (tls_key_path, tls_cert_path) {
        (Some(tls_key_path), Some(tls_cert_path)) => {
            tracing::info!("🔐 Private key {}", tls_key_path.display(),);
            tracing::info!("🔒 Public key {}", tls_cert_path.display());

            #[cfg(feature = "rustls")]
            return Ok(Some(
                axum_server::tls_rustls::RustlsConfig::from_pem_file(tls_cert_path, tls_key_path)
                    .await
                    .with_context(|| "loading TLS cert/key failed")?
                    .into(),
            ));

            #[cfg(feature = "native-tls")]
            return Ok(Some(
                axum_server::tls_openssl::OpenSSLConfig::from_pem_file(tls_cert_path, tls_key_path)
                    .with_context(|| "loading TLS cert/key failed")?
                    .into(),
            ));

            bail!("TLS configuration was requested, but no TLS provider was enabled during compilation")
        }
        (None, Some(_)) => Err(anyhow!("TLS cert path provided without key path")),
        (Some(_), None) => Err(anyhow!("TLS key path provided without cert path")),
        (None, None) => Ok(None),
    }
}

fn absolute_path_if_some(
    maybe_path: Option<PathBuf>,
    file_description: &str,
) -> anyhow::Result<Option<PathBuf>, anyhow::Error> {
    match maybe_path {
        Some(path) => {
            let path = if path.to_string_lossy().contains('~') {
                let home_path = homedir::my_home()
                    .context("home directory path not available")?
                    .context("no home directory")?;
                let new_path = path
                    .to_string_lossy()
                    .replace('~', &home_path.to_string_lossy());
                PathBuf::from(new_path)
            } else {
                path
            };
            Ok(Some(absolute_path(path, file_description)?))
        }
        None => Ok(None),
    }
}

fn absolute_path(path: PathBuf, file_description: &str) -> anyhow::Result<PathBuf, anyhow::Error> {
    path.canonicalize().with_context(|| {
        format!(
            "error getting canonical path to {} file {:?}",
            file_description, &path
        )
    })
}
