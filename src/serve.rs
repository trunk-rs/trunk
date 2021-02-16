use std::sync::Arc;

use anyhow::Result;
use indicatif::ProgressBar;
use tokio::task::JoinHandle;
use warp::{Filter, Rejection, Reply};

use crate::common::{ERROR, SERVER};
use crate::config::RtcServe;
use crate::proxy::{http_proxy, ws_proxy, ProxyRejection};
use crate::watch::WatchSystem;
use warp::http::status::StatusCode;

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
        let watch_handle = tokio::spawn(self.watch.run());
        let server_handle = Self::spawn_server(self.cfg.clone(), self.http_addr.clone(), self.progress.clone());

        // Open the browser.
        if self.cfg.open {
            if let Err(err) = open::that(self.http_addr) {
                self.progress.println(format!("error opening browser: {}", err));
            }
        }

        let _ = server_handle.await;
        let _ = watch_handle.await;
        Ok(())
    }

    fn spawn_server(cfg: Arc<RtcServe>, http_addr: String, progress: ProgressBar) -> JoinHandle<()> {
        // Build proxies.
        let mut proxy_route = dummy().boxed();

        if let Some(backend) = &cfg.proxy_backend {
            let path = cfg.proxy_path.clone().unwrap_or_else(String::new);
            let mut paths = warp::any().boxed();
            for path in path.split('/').into_iter().map(|it| it.to_owned()) {
                paths = paths.and(warp::path(path)).boxed();
            }

            let proxy_to = backend.to_string();

            if cfg.proxy_ws {
                progress.println(format!("{} proxying websocket /{} -> {}", SERVER, path, proxy_to));
                proxy_route = ws_proxy(paths, proxy_to).map(Reply::into_response).boxed();
            } else {
                progress.println(format!("{} proxying http /{} -> {}", SERVER, path, proxy_to));
                proxy_route = http_proxy(paths, proxy_to).map(Reply::into_response).boxed();
            };
        } else if let Some(proxies) = &cfg.proxies {
            for proxy_config in proxies {
                let path = proxy_config.path.clone().unwrap_or_else(String::new);
                let proxy_to = proxy_config.backend.to_string();

                // `warp::path` requires that `/` must not be inside the passed path so we
                // remove that and handle segments with `and`ing the `warp::path` for each segment
                let mut paths = warp::any().boxed();
                for path in path.split('/').into_iter().map(|it| it.to_owned()) {
                    paths = paths.and(warp::path(path)).boxed();
                }

                if proxy_config.ws {
                    progress.println(format!("{} proxying websocket /{} -> {}", SERVER, path, proxy_to));
                    proxy_route = proxy_route.or(ws_proxy(paths, proxy_to)).map(Reply::into_response).boxed();
                } else {
                    progress.println(format!("{} proxying http /{} -> {}", SERVER, path, proxy_to));
                    proxy_route = proxy_route.or(http_proxy(paths, proxy_to)).map(Reply::into_response).boxed();
                };
            }
        };

        let routes = warp::fs::dir(cfg.watch.build.final_dist.clone())
            .or(proxy_route)
            .or(warp::fs::file(cfg.watch.build.final_dist.join("index.html")))
            .recover(rejection_handler);

        // Listen and serve.
        progress.println(format!("{} server running at {}\n", SERVER, &http_addr));
        tokio::spawn(async move {
            warp::serve(routes).run(([0, 0, 0, 0], cfg.port)).await;
        })
    }
}

// workaround to make compiler happy
fn dummy() -> impl Filter<Extract = (warp::reply::Response,), Error = Rejection> + Clone + Sync + Send {
    warp::any().and_then(|| async move { Err(warp::reject()) })
}

async fn rejection_handler(err: Rejection) -> Result<impl warp::Reply, Rejection> {
    let mut error = "".to_string();
    if let Some(e) = err.find::<ProxyRejection>() {
        eprintln!("{} proxy error: {}", ERROR, e.0);
        error = e.0.to_string();
    }

    Ok(warp::reply::with_status(error, StatusCode::INTERNAL_SERVER_ERROR))
}
