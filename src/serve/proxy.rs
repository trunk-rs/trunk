use super::SERVER;
use crate::proxy::{ProxyHandlerHttp, ProxyHandlerWebSocket};
use anyhow::Context;
use axum::http::Uri;
use axum::Router;
use console::Emoji;
use http::HeaderMap;
use reqwest::redirect::Policy;
use reqwest::Client;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

const DANGER: Emoji = Emoji("⚠️", "(!)");

/// A builder for the proxy router
pub(crate) struct ProxyBuilder {
    tls: bool,
    router: Router,
    clients: ProxyClients,
}

impl ProxyBuilder {
    /// Create a new builder
    pub fn new(tls: bool, router: Router) -> Self {
        Self {
            tls,
            router,
            clients: Default::default(),
        }
    }

    /// Register a new proxy config
    pub fn register_proxy(
        mut self,
        ws: bool,
        backend: &Uri,
        request_headers: &HeaderMap,
        rewrite: Option<String>,
        opts: ProxyClientOptions,
    ) -> anyhow::Result<Self> {
        let proto = match self.tls {
            true => "https",
            false => "http",
        }
        .to_string();

        if ws {
            let handler = ProxyHandlerWebSocket::new(
                proto,
                backend.clone(),
                request_headers.clone(),
                rewrite,
            );
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
            let handler = ProxyHandlerHttp::new(
                proto,
                client,
                backend.clone(),
                request_headers.clone(),
                rewrite,
            );
            tracing::info!(
                "{}proxying {} -> {} {} {}{}",
                SERVER,
                handler.path(),
                &backend,
                &request_headers
                    .iter()
                    .map(|(header_name, header_value)| format!("{header_name}={header_value:?}"))
                    .collect::<Vec<String>>()
                    .join(";"),
                if no_sys_proxy {
                    "; ignoring system proxy"
                } else {
                    ""
                },
                if insecure {
                    format!("; {DANGER}️ insecure TLS")
                } else {
                    Default::default()
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
    pub redirect: bool,
}

#[derive(Default)]
pub(crate) struct ProxyClients {
    clients: HashMap<ProxyClientOptions, Client>,
}

impl ProxyClients {
    pub fn get_client(&mut self, opts: ProxyClientOptions) -> anyhow::Result<Client> {
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
    fn create_client(opts: ProxyClientOptions) -> anyhow::Result<Client> {
        let mut builder = reqwest::ClientBuilder::new()
            .http1_only()
            .redirect(if opts.redirect {
                Policy::default()
            } else {
                Policy::none()
            });

        #[cfg(any(feature = "native-tls", feature = "rustls"))]
        if opts.insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }

        if opts.no_system_proxy {
            builder = builder.no_proxy();
        }
        builder.build().context("error building proxy client")
    }
}
