use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use async_std::task::{spawn, spawn_local, JoinHandle};
use console::Emoji;
use indicatif::ProgressBar;
use structopt::StructOpt;
use tide::{Request, Response, Middleware, Next, StatusCode};
use tide::http::mime;

use crate::config::{ConfigOpts, ConfigOptsBuild, ConfigOptsWatch, ConfigOptsServe, RtcServe};
use crate::watch::WatchSystem;

/// Build the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name="serve")]
pub struct Serve {
    #[structopt(flatten)]
    pub build: ConfigOptsBuild,
    #[structopt(flatten)]
    pub watch: ConfigOptsWatch,
    #[structopt(flatten)]
    pub serve: ConfigOptsServe,
}

impl Serve {
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let cfg = ConfigOpts::rtc_serve(self.build, self.watch, self.serve, config).await?;

        // Build the watcher system.
        let mut watcher = WatchSystem::new(cfg.watch.clone()).await?;
        watcher.build().await;
        let progress = watcher.get_progress_handle();

        // Spawn the watcher & the server.
        let http_addr = format!("http://127.0.0.1:{}{}", cfg.port, &cfg.watch.build.public_url);
        let watch_handle = spawn_local(watcher.run());
        let server_handle = Self::spawn_server(&cfg, http_addr.clone(), progress.clone())?;

        // Open the browser.
        if cfg.open {
            if let Err(err) = open::that(http_addr) {
                progress.println(format!("error opening browser: {}", err));
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
        let mut app = tide::with_state(State{index});
        app.with(IndexHtmlMiddleware);
        app.at(&cfg.watch.build.public_url).serve_dir(cfg.watch.build.dist.to_string_lossy().as_ref())?;

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
