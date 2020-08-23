use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use async_std::fs;
use structopt::StructOpt;
use tide::{Request, Response, Middleware, Next, StatusCode};
use tide::http::mime;

/// Build the Rust WASM app and all of its assets.
#[derive(StructOpt)]
#[structopt(name="serve")]
pub struct Serve {
    /// The asset dir to serve from.
    #[structopt(short, long, default_value="dist", parse(from_os_str))]
    dist: PathBuf,
    /// The port to serve on.
    #[structopt(short, long, default_value="8080")]
    port: u16,
}

impl Serve {
    pub async fn run(&self) -> Result<()> {
        // Prep state.
        let listen_addr = format!("0.0.0.0:{}", self.port);
        let index = Arc::new(self.dist.join("index.html"));

        // Build app.
        let mut app = tide::with_state(State{index});
        app.with(IndexHtmlMiddleware);
        app.at("/").serve_dir(self.dist.to_string_lossy().as_ref())?;

        // Listen and serve.
        println!("📡 {}", format!("listening at http://{}", &listen_addr));
        app.listen(listen_addr).await?;
        Ok(())
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
            StatusCode::NotFound => Response::builder(404)
                .content_type(mime::HTML)
                .body(load_index_html(&index).await?)
                .build(),
            _ => res,
        })
    }
}
