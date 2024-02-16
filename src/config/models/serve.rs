use crate::config::models::AddressFamily;
use crate::config::WsProtocol;
use axum::http::Uri;
use clap::Args;
use serde::Deserialize;
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::PathBuf;

/// Config options for the serve system.
#[derive(Clone, Debug, Default, Deserialize, Args)]
#[command(next_help_heading = "Serve")]
pub struct ConfigOptsServe {
    /// A single address to serve on.
    // This is required for the TOML to allow a single "address" field as before
    #[arg(skip)]
    pub address: Option<IpAddr>,
    /// The addresses to serve on [default: <local>]
    #[arg(id = "address", long)]
    pub addresses: Option<Vec<IpAddr>>,
    #[arg(short = 'A', long, env)]
    #[serde(default)]
    pub prefer_address_family: Option<AddressFamily>,
    /// The port to serve on [default: 8080]
    #[arg(long)]
    pub port: Option<u16>,
    /// Open a browser tab once the initial build is complete [default: false]
    #[arg(long)]
    #[serde(default)]
    pub open: bool,
    /// A URL to which requests will be proxied [default: None]
    #[arg(long = "proxy-backend")]
    #[serde(default, deserialize_with = "super::deserialize_uri")]
    pub proxy_backend: Option<Uri>,
    /// The URI on which to accept requests which are to be rewritten and proxied to backend
    /// [default: None]
    #[arg(long = "proxy-rewrite")]
    #[serde(default)]
    pub proxy_rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets [default: false]
    #[arg(long = "proxy-ws")]
    #[serde(default)]
    pub proxy_ws: bool,
    /// Configure the proxy to accept insecure requests [default: false]
    #[arg(long = "proxy-insecure")]
    #[serde(default)]
    pub proxy_insecure: bool,
    /// Configure the proxy to bypass system proxy [default: false]
    #[arg(long = "proxy-no-system-proxy")]
    #[serde(default)]
    pub proxy_no_system_proxy: bool,
    /// Disable auto-reload of the web app [default: false]
    #[arg(long = "no-autoreload")]
    #[serde(default)]
    pub no_autoreload: bool,
    /// Additional headers to send in responses [default: none]
    #[clap(skip)]
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Disable error reporting in the browser [default: false]
    #[arg(long = "no-error-reporting")]
    #[serde(default)]
    pub no_error_reporting: bool,
    /// Disable fallback to index.html for missing files [default: false]
    #[arg(long = "no-spa")]
    #[serde(default)]
    pub no_spa: bool,
    /// Protocol used for the auto-reload WebSockets connection [enum: ws, wss]
    #[arg(long = "ws-protocol")]
    pub ws_protocol: Option<WsProtocol>,
    /// The path to the trunk web-socket [default: <serve-base>]
    #[arg(long)]
    pub ws_base: Option<String>,
    /// The TLS key file to enable TLS encryption [default: None]
    #[arg(long)]
    pub tls_key_path: Option<PathBuf>,
    /// The TLS cert file to enable TLS encryption [default: None]
    #[arg(long)]
    pub tls_cert_path: Option<PathBuf>,
    /// A base path to serve the application from [default: <public-url>]
    #[arg(long)]
    pub serve_base: Option<String>,
}
