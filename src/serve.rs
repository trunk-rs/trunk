use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use tokio::task::{spawn, JoinHandle};
use warp::{http, Filter, Rejection};

use crate::common::SERVER;
use crate::config::RtcServe;
use crate::watch::WatchSystem;
use std::str::FromStr;
use warp::http::{Request, StatusCode, Uri};

/// A system encapsulating a build & watch system, responsible for serving generated content.
pub struct ServeSystem {
    cfg: Arc<RtcServe>,
    watch: WatchSystem,
    http_addr: String,
    progress: ProgressBar,
}

impl ServeSystem {
    /// Construct a new instance.
    pub async fn new(cfg: Arc<RtcServe>, progress: ProgressBar) -> Result<Self> {
        let watch = WatchSystem::new(cfg.watch.clone(), progress.clone()).await?;
        let http_addr = format!("http://127.0.0.1:{}{}", cfg.port, &cfg.watch.build.public_url);
        Ok(Self {
            cfg,
            watch,
            http_addr,
            progress,
        })
    }

    /// Run the serve system.
    pub async fn run(mut self) -> Result<()> {
        // Spawn the watcher & the server.
        self.watch.build().await;
        let watch_handle = self.watch.run();
        let server_handle = Self::spawn_server(self.cfg.clone(), self.http_addr.clone(), self.progress.clone())?;

        // Open the browser.
        if self.cfg.open {
            if let Err(err) = open::that(self.http_addr) {
                self.progress.println(format!("error opening browser: {}", err));
            }
        }

        tokio::spawn(server_handle);
        watch_handle.await;
        Ok(())
    }

    fn spawn_server(cfg: Arc<RtcServe>, http_addr: String, progress: ProgressBar) -> Result<JoinHandle<()>> {
        // Build app.
        let routes = warp::fs::dir(cfg.watch.build.dist.clone());

        let mut proxy_route = dummy().boxed();
        // Build proxies.
        if let Some(backend) = &cfg.proxy_backend {
            let proxy = proxy("".to_string(), backend.to_string());
            proxy_route = proxy.boxed();
        } else if let Some(proxies) = &cfg.proxies {
            for proxy_config in proxies {
                let proxy = proxy(proxy_config.rewrite.clone().unwrap_or_else(String::new), proxy_config.backend.to_string());
                proxy_route = proxy.boxed();
            }
        };

        let routes = routes.or(proxy_route).or(warp::fs::file(cfg.watch.build.dist.join("index.html")));

        // Listen and serve.
        progress.println(format!("{} server running at {}\n", SERVER, &http_addr));
        Ok(spawn(async move {
            warp::serve(routes).run(([0, 0, 0, 0], cfg.port)).await;
        }))
    }
}

// workaround to make compiler happy
fn dummy() -> impl Filter<Extract = (warp::reply::Response,), Error = Rejection> + Clone {
    warp::any().and_then(|| async move { Err(warp::reject()) })
}

pub fn extract_request() -> impl Filter<Extract = (http::Request<warp::hyper::Body>,), Error = warp::Rejection> + Copy {
    warp::method()
        .and(warp::path::full())
        .and(warp::header::headers_cloned())
        .and(warp::body::bytes())
        .map(|method: http::Method, path: warp::path::FullPath, headers: http::HeaderMap, body| {
            let mut req = http::Request::builder()
                .method(method)
                .uri(path.as_str())
                .body(warp::hyper::Body::from(body))
                .expect("request builder");
            {
                *req.headers_mut() = headers;
            }
            req
        })
}

async fn and_then_handle(mut request: Request<warp::hyper::Body>, proxy_to: String) -> Result<warp::reply::Response, warp::Rejection> {
    let client = warp::hyper::client::Client::new();
    let uri = request.uri();
    let proxy_to = proxy_to.strip_suffix("/").unwrap_or(&proxy_to);

    *request.uri_mut() = Uri::from_str(&format!("{}{}", proxy_to, uri)).unwrap();

    println!("uri: {}", request.uri());

    let resp = client.request(request).await;
    let resp = resp.unwrap();
    println!("resp: {:?}", resp);

    if resp.status() == StatusCode::SWITCHING_PROTOCOLS {
        println!("websocket upgrade");
        // todo somehow forward messages
        // i think i have to establish connection to the server
        // listen to messages from client and forward those to the server
        // same goes for other way - listen to server, send to client
        // how? i got no idea

        Ok(resp)
    } else {
        Ok(resp)
    }
}

fn proxy(path: String, proxy_to: String) -> impl Filter<Extract = (warp::reply::Response,), Error = warp::Rejection> + Clone {
    if path == "" { warp::any().boxed() } else { warp::path(path).boxed() }
        .and(extract_request())
        .and(warp::any().map(move || proxy_to.clone()))
        .and_then(and_then_handle)
}
