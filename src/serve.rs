use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use async_std::task::{spawn, spawn_local, JoinHandle};
use console::Emoji;
use indicatif::ProgressBar;
use tide::http::mime;
use tide::{Middleware, Next, Request, Response, StatusCode};

use crate::config::RtcServe;
use crate::watch::WatchSystem;

/// A system encapsulating a build & watch system, responsible for serving generated content.
pub struct ServeSystem {
    cfg: Arc<RtcServe>,
    watch: WatchSystem,
    progress: ProgressBar,
    http_addr: String,
}

impl ServeSystem {
    /// Construct a new instance.
    pub async fn new(cfg: Arc<RtcServe>) -> Result<Self> {
        let watch = WatchSystem::new(cfg.watch.clone()).await?;
        let progress = watch.get_progress_handle();
        let http_addr = format!("http://127.0.0.1:{}{}", cfg.port, &cfg.watch.build.public_url);
        Ok(Self {
            cfg,
            watch,
            progress,
            http_addr,
        })
    }

    /// Run the serve system.
    pub async fn run(mut self) -> Result<()> {
        // Spawn the watcher & the server.
        self.watch.build().await;
        let watch_handle = spawn_local(self.watch.run());
        let server_handle = Self::spawn_server(&self.cfg, self.http_addr.clone(), self.progress.clone())?;

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

    fn spawn_server(cfg: &RtcServe, http_addr: String, progress: ProgressBar) -> Result<JoinHandle<()>> {
        // Prep state.
        let listen_addr = format!("0.0.0.0:{}", cfg.port);
        let index = Arc::new(cfg.watch.build.dist.join("index.html"));

        // Build app.
        let mut app = tide::with_state(State { index });
        app.with(IndexHtmlMiddleware);
        app.at(&cfg.watch.build.public_url)
            .serve_dir(cfg.watch.build.dist.to_string_lossy().as_ref())?;

        // Listen and serve.
        progress.println(format!("{}server running at {}\n", Emoji("📡 ", "  "), &http_addr));
        Ok(spawn(async move {
            if let Err(err) = app.listen(listen_addr).await {
                progress.println(format!("{}", err));
            }
        }))
    }
}

/// Server state.
#[derive(Clone)]
struct State {
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
