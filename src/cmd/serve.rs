use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use async_std::task::{spawn, spawn_local, JoinHandle};
use structopt::StructOpt;
use tide::{Request, Response, Middleware, Next, StatusCode};
use tide::http::mime;

use crate::common::parse_public_url;
use crate::watch::WatchSystem;

/// Build the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name="serve")]
pub struct Serve {
    /// The index HTML file to drive the bundling process.
    #[structopt(default_value="index.html", parse(from_os_str))]
    target: PathBuf,

    /// The port to serve on.
    #[structopt(short, long, default_value="8080")]
    port: u16,
    /// Build in release mode.
    #[structopt(long)]
    release: bool,
    /// The output dir for all final assets.
    #[structopt(short, long, default_value="dist", parse(from_os_str))]
    dist: PathBuf,
    /// The public URL from which assets are to be served.
    #[structopt(long, default_value="/", parse(from_str=parse_public_url))]
    public_url: String,
    /// Additional paths to ignore.
    #[structopt(short, long, parse(from_os_str))]
    ignore: Option<Vec<PathBuf>>,
    /// Open a browser tab once the initial build is complete.
    #[structopt(long)]
    open: bool,
}

impl Serve {
    pub async fn run(self) -> Result<()> {
        let (target, release, dist, public_url, ignore) = (
            self.target.clone(), self.release, self.dist.clone(), self.public_url.clone(), self.ignore.clone().unwrap_or_default(),
        );
        let mut watcher = WatchSystem::new(target, release, dist, public_url, ignore).await?;
        watcher.build().await;

        // Spawn the watcher & the server.
        let http_addr = format!("http://127.0.0.1:{}{}", self.port, &self.public_url);
        let watch_handle = spawn_local(watcher.run());
        let server_handle = self.spawn_server(http_addr.clone())?;

        // Open the browser.
        if self.open {
            if let Err(err) = open::that(http_addr) {
                eprintln!("error opening browser: {}", err);
            }
        }

        server_handle.await;
        watch_handle.await;
        Ok(())
    }

    fn spawn_server(&self, http_addr: String) -> Result<JoinHandle<()>> {
        // Prep state.
        let listen_addr = format!("0.0.0.0:{}", self.port);
        let index = Arc::new(self.dist.join("index.html"));

        // Build app.
        let mut app = tide::with_state(State{index});
        app.with(IndexHtmlMiddleware);
        app.at(&self.public_url).serve_dir(self.dist.to_string_lossy().as_ref())?;

        // Listen and serve.
        println!("Server running at {}", &http_addr);
        Ok(spawn(async move {
            if let Err(err) = app.listen(listen_addr).await {
                eprintln!("{}", err);
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
