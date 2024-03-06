use crate::config::{
    models::AddressFamily, BaseUrl, ConfigOptsBuild, ConfigOptsCore, ConfigOptsHook,
    ConfigOptsProxy, ConfigOptsServe, ConfigOptsTools, ConfigOptsWatch, WsProtocol,
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use axum::http::Uri;
use local_ip_address::list_afinet_netifas;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::log;

use crate::tls::TlsConfig;

/// Runtime config for the serve system.
#[derive(Clone, Debug)]
pub struct RtcServe {
    /// Runtime config for the watch system.
    pub watch: Arc<super::RtcWatch>,
    /// The IP address to serve on.
    pub addresses: Vec<IpAddr>,
    /// The port to serve on.
    pub port: u16,
    /// Open a browser tab once the initial build is complete.
    pub open: bool,
    /// A URL to which requests will be proxied.
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend.
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets.
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure connections.
    pub proxy_insecure: bool,
    /// Configure the proxy to bypass system proxy.
    pub proxy_no_sys_proxy: bool,
    /// Any proxies configured to run along with the server.
    pub proxies: Option<Vec<ConfigOptsProxy>>,
    /// Whether to disable auto-reload of the web page when a build completes.
    pub no_autoreload: bool,
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
}

impl RtcServe {
    pub(crate) async fn new(
        core_opts: ConfigOptsCore,
        build_opts: ConfigOptsBuild,
        watch_opts: ConfigOptsWatch,
        opts: ConfigOptsServe,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        proxies: Option<Vec<ConfigOptsProxy>>,
    ) -> Result<Self> {
        let watch = Arc::new(super::RtcWatch::new(
            core_opts,
            build_opts,
            watch_opts,
            tools,
            hooks,
            !opts.no_autoreload,
            opts.no_error_reporting,
        )?);
        let tls = tls_config(
            absolute_path_if_some(opts.tls_key_path, "tls_key_path")?,
            absolute_path_if_some(opts.tls_cert_path, "tls_cert_path")?,
        )
        .await?;

        let addresses = opts
            .address
            .into_iter()
            .chain(opts.addresses.into_iter().flatten())
            .collect::<Vec<_>>();

        Ok(Self {
            watch,
            addresses: build_address_list(opts.prefer_address_family, addresses),
            port: opts.port.unwrap_or(8080),
            open: opts.open,
            proxy_backend: opts.proxy_backend,
            proxy_rewrite: opts.proxy_rewrite,
            proxy_insecure: opts.proxy_insecure,
            proxy_no_sys_proxy: opts.proxy_no_system_proxy,
            proxy_ws: opts.proxy_ws,
            proxies,
            no_autoreload: opts.no_autoreload,
            no_spa: opts.no_spa,
            headers: opts.headers,
            ws_protocol: opts.ws_protocol,
            ws_base: opts.ws_base,
            tls,
            serve_base: opts.serve_base,
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
        Some(path) => Ok(Some(absolute_path(path, file_description)?)),
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
