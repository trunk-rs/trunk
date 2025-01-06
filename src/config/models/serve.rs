use crate::config::{
    models::ConfigModel,
    types::{AddressFamily, Uri, WsProtocol},
};
use schemars::JsonSchema;
use serde::Deserialize;
use std::{collections::HashMap, net::IpAddr, path::PathBuf};
use tracing::log;

/// Config options for the serve system.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Serve {
    /// A single address to serve on.
    // This is required for the TOML to allow a single "address" field as before
    #[serde(default)]
    #[deprecated(note = "Use the 'addresses' field instead")]
    pub address: Option<IpAddr>,
    /// The addresses to serve on [default: <local loopback>]
    #[serde(default)]
    pub addresses: Vec<IpAddr>,
    #[serde(default)]
    pub prefer_address_family: Option<AddressFamily>,
    /// Disable the reverse DNS lookup during startup
    #[serde(default)]
    pub disable_address_lookup: bool,
    /// The port to serve on [default: 8080]
    #[serde(default = "default::port")]
    pub port: u16,
    /// The aliases to serve on.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[serde(default)]
    pub open: bool,
    /// Disable auto-reload of the web app
    #[serde(default)]
    pub no_autoreload: bool,
    /// Additional headers to send in responses
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Disable error reporting in the browser
    #[serde(default)]
    pub no_error_reporting: bool,
    /// Disable fallback to index.html for missing files
    #[serde(default)]
    pub no_spa: bool,
    /// Protocol used for the auto-reload WebSockets connection
    pub ws_protocol: Option<WsProtocol>,
    /// The path to the trunk web-socket
    #[serde(default)]
    pub ws_base: Option<String>,
    /// The TLS key file to enable TLS encryption
    #[serde(default)]
    pub tls_key_path: Option<PathBuf>,
    /// The TLS cert file to enable TLS encryption
    #[serde(default)]
    pub tls_cert_path: Option<PathBuf>,
    /// A base path to serve the application from
    #[serde(default)]
    pub serve_base: Option<String>,
    /// Configure the proxy to not follow redirects
    #[serde(default)]
    pub proxy_no_redirect: Option<bool>,

    /// A URL to which requests will be proxied [default: None]
    #[deprecated]
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[serde(default)]
    #[deprecated]
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets
    #[serde(default)]
    #[deprecated]
    pub proxy_ws: Option<bool>,
    /// Configure the proxy to accept insecure requests
    #[serde(default)]
    #[deprecated]
    pub proxy_insecure: Option<bool>,
    /// Configure the proxy to bypass system proxy
    #[serde(default)]
    #[deprecated]
    pub proxy_no_system_proxy: Option<bool>,
    /// Disable CSP header
    #[serde(default)]
    pub disable_csp: bool,
    /// The CSP;  {{NONE}} is replaced by a random nonce
    #[serde(default = "default::csp")]
    pub csp: Vec<String>,
}

impl Default for Serve {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            address: None,
            addresses: vec![],
            aliases: vec![],
            prefer_address_family: None,
            port: default::port(),
            disable_address_lookup: false,
            open: false,
            no_autoreload: false,
            headers: Default::default(),
            no_error_reporting: false,
            no_spa: false,
            ws_protocol: None,
            ws_base: None,
            tls_key_path: None,
            tls_cert_path: None,
            serve_base: None,
            proxy_backend: None,
            proxy_rewrite: None,
            proxy_ws: None,
            proxy_insecure: None,
            proxy_no_system_proxy: None,
            proxy_no_redirect: None,
            disable_csp: false,
            csp: default::csp(),
        }
    }
}

mod default {
    pub const fn port() -> u16 {
        8080
    }

    pub fn csp() -> Vec<String> {
        [
            "script-src 'wasm-unsafe-eval' 'nonce-{{NONCE}}'",
            "style-src 'nonce-{{NONCE}}'",
        ]
        .map(|s| s.to_string())
        .into()
    }
}

macro_rules! check_proxy_setting {
    ($s: expr, $f: ident) => {
        if $s.$f.is_some() {
            log::warn!(
                "Found a setting for single {}, without single proxy_rewrite setting. This has no effect.", stringify!($f)
            );
        }
    };
}

impl ConfigModel for Serve {
    #[allow(deprecated)]
    fn migrate(&mut self) -> anyhow::Result<()> {
        if let Some(address) = self.address.take() {
            log::warn!("The field `address` in the configuration is deprecated and will be removed in a future version. Migrate to the `addresses` field, which allows adding more than one.");
            self.addresses.push(address);
        }

        // only the proxy_backend triggers the addition, warn if it is missing but others are present
        if self.proxy_backend.is_none() {
            check_proxy_setting!(self, proxy_rewrite);
            check_proxy_setting!(self, proxy_ws);
            check_proxy_setting!(self, proxy_insecure);
            check_proxy_setting!(self, proxy_no_system_proxy);
        }

        Ok(())
    }
}
