use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use async_std::task::{spawn, spawn_local, JoinHandle};
use indicatif::ProgressBar;
use tide::http::mime;
use tide::{Middleware, Next, Request, Response, StatusCode};

use crate::common::SERVER;
use crate::config::RtcServe;
use crate::proxy::ProxyHandlerHttp;
use crate::watch::WatchSystem;

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
        let watch_handle = spawn_local(self.watch.run());
        let server_handle = Self::spawn_server(self.cfg.clone(), self.http_addr.clone(), self.progress.clone())?;

        // Open the browser.
        if self.cfg.open {
            if let Err(err) = open::that(self.http_addr) {
                self.progress.println(format!("error opening browser: {}", err));
            }
        }

        server_handle.await;
        watch_handle.await;
        Ok(())
    }

    fn spawn_server(cfg: Arc<RtcServe>, http_addr: String, progress: ProgressBar) -> Result<JoinHandle<()>> {
        // Prep state.
        let listen_addr = format!("0.0.0.0:{}", cfg.port);
        let index = Arc::new(cfg.watch.build.final_dist.join("index.html"));

        // Build app.
        tide::log::with_level(tide::log::LevelFilter::Error);
        let mut app = tide::with_state(State { index });
        app.with(IndexHtmlMiddleware)
            .at(&cfg.watch.build.public_url)
            .serve_dir(cfg.watch.build.final_dist.to_string_lossy().as_ref())?;

        // Build proxies.
        if let Some(backend) = &cfg.proxy_backend {
            let handler = Arc::new(ProxyHandlerHttp::new(backend.clone(), cfg.proxy_rewrite.clone()));
            progress.println(format!("{} proxying {} -> {}\n", SERVER, handler.path(), &backend));
            app.at(handler.path()).strip_prefix().all(move |req| {
                let handler = handler.clone();
                async move { handler.proxy_request(req).await }
            });
        } else if let Some(proxies) = &cfg.proxies {
            for proxy in proxies.iter() {
                let handler = Arc::new(ProxyHandlerHttp::new(proxy.backend.clone(), proxy.rewrite.clone()));
                progress.println(format!("{} proxying {} -> {}\n", SERVER, handler.path(), &proxy.backend));
                app.at(handler.path()).strip_prefix().all(move |req| {
                    let handler = handler.clone();
                    async move { handler.proxy_request(req).await }
                });
            }
        }

        // Listen and serve.
        progress.println(format!("{} server running at {}\n", SERVER, &http_addr));
        Ok(spawn(async move {
            if let Err(err) = app.listen(listen_addr).await {
                progress.println(err.to_string());
            }
        }))
    }
}

/// Server state.
#[derive(Clone, Debug)]
pub struct State {
    /// The path to the index.html file.
    pub index: Arc<PathBuf>,
}

async fn load_index_html(index: &Path) -> tide::Result<Vec<u8>> {
    Ok(fs::read(index).await?)
}

/// Middleware for accessing the index.html from any request which needs it.
struct IndexHtmlMiddleware;

#[tide::utils::async_trait]
impl Middleware<State> for IndexHtmlMiddleware {
    async fn handle(&self, req: Request<State>, next: Next<'_, State>) -> tide::Result {
        let index = req.state().index.clone();
        let res = next.run(req).await;
        Ok(match res.status() {
            StatusCode::NotFound => Response::builder(StatusCode::Ok)
                .content_type(mime::HTML)
                .body(load_index_html(&index).await?)
                .build(),
            _ => res,
        })
    }
}
