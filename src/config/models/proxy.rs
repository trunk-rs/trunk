use axum::http::Uri;
use serde::Deserialize;

/// Config options for building proxies.
///
/// NOTE WELL: this configuration type is different from the others inasmuch as it is only used
/// when parsing the `Trunk.toml` config file. It is not intended to be configured via CLI or env
/// vars.
#[derive(Clone, Debug, Deserialize)]
pub struct ConfigOptsProxy {
    /// The URL of the backend to which requests are to be proxied.
    #[serde(deserialize_with = "super::deserialize_uri")]
    pub backend: Uri,
    /// An optional URI prefix which is to be used as the base URI for proxying requests, which
    /// defaults to the URI of the backend.
    ///
    /// When a value is specified, requests received on this URI will have this URI segment
    /// replaced with the URI of the `backend`.
    pub rewrite: Option<String>,
    /// Configure the proxy for handling WebSockets.
    #[serde(default)]
    pub ws: bool,
    /// Configure the proxy to accept insecure certificates.
    #[serde(default)]
    pub insecure: bool,
    /// Configure the proxy to bypass the system proxy. Defaults to `false`.
    #[serde(rename = "no-system-proxy")]
    #[serde(default)]
    pub no_system_proxy: bool,
}
