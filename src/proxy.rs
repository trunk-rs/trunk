use http_types::Url;
use tide::{Request, Result};

use crate::serve::State;

/// A handler used for proxying HTTP requests to a backend.
pub struct ProxyHandlerHttp {
    /// The URL of the backend to which requests are to be proxied.
    backend: Url,
    /// An optional rewrite path to be used as the listening URI prefix, but which will be
    /// stripped before being sent to the proxy backend.
    rewrite: Option<String>,
}

impl ProxyHandlerHttp {
    /// Create a new instance.
    pub fn new(backend: Url, rewrite: Option<String>) -> Self {
        Self { backend, rewrite }
    }

    /// The path on which this proxy handler is to listen.
    pub fn path(&self) -> &str {
        self.rewrite.as_ref().map(AsRef::as_ref).unwrap_or_else(|| self.backend.path())
    }

    /// Proxy the given request to the target backend.
    pub async fn proxy_request(&self, mut req: Request<State>) -> Result {
        // Build a new request to be sent to the proxy backend.
        let req_url = req.url();
        let req_path = req_url.path();
        let mut url = self.backend.clone();
        if let Ok(mut segments) = url.path_segments_mut() {
            segments.pop_if_empty().extend(req_path.trim_start_matches('/').split('/'));
        }
        url.set_query(req_url.query());
        let mut request = surf::RequestBuilder::new(req.method(), url).body(req.take_body());
        for (hname, hval) in req.iter() {
            request = request.header(hname, hval);
        }
        // Ensure the host header is set to target the backend itself.
        if let Some(host) = self.backend.host_str() {
            request = request.header("host", host);
        }

        // Send the request & unpack the response.
        let mut res = request.send().await?;
        let mut response = tide::Response::builder(res.status()).body(res.take_body());
        for (hname, hval) in res.iter() {
            response = response.header(hname, hval);
        }
        Ok(response.build())
    }
}
