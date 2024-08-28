use std::collections::HashMap;

use crate::{config::models::ConfigModel, config::types::Uri};
use schemars::JsonSchema;
use serde::Deserialize;

/// Config options for building proxies.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Proxy {
    /// The URL of the backend to which requests are to be proxied.
    pub backend: Uri,
    /// An optional URI prefix which is to be used as the base URI for proxying requests, which
    /// defaults to the URI of the backend.
    ///
    /// When a value is specified, requests received on this URI will have this URI segment
    /// replaced with the URI of the `backend`.
    pub rewrite: Option<String>,
    /// A set of headers to pass to the proxied backend.
    #[serde(default)]
    pub request_headers: HashMap<String, String>,
    /// Configure the proxy for handling WebSockets.
    #[serde(default)]
    pub ws: bool,
    /// Configure the proxy to accept insecure certificates (danger!).
    #[serde(default)]
    pub insecure: bool,
    /// Configure the proxy to bypass the system proxy.
    #[serde(alias = "no-system-proxy")]
    #[serde(default)]
    pub no_system_proxy: bool,
    /// Automatically redirect proxy requests? `no_redirect` defaults to
    /// `false`, i.e. yes, follow redirects automatically.
    #[serde(default)]
    pub no_redirect: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
pub struct Proxies(pub Vec<Proxy>);

impl ConfigModel for Proxies {}
