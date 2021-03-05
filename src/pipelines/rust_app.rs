//! Rust application pipeline.

use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{ffi::OsStr, str::FromStr};

use anyhow::{anyhow, bail, ensure, Context, Result};
use async_process::{Command, Stdio};
use async_std::fs;
use async_std::task::{spawn, JoinHandle};
use futures::channel::mpsc::Sender;
use indicatif::ProgressBar;
use nipper::Document;

use super::{LinkAttrs, TrunkLinkPipelineOutput};
use super::{ATTR_HREF, SNIPPETS_DIR};
use crate::common::{copy_dir_recursive, path_exists};
use crate::config::{CargoMetadata, RtcBuild};

/// A Rust application pipeline.
pub struct RustApp {
    /// The ID of this pipeline's source HTML element.
    id: Option<usize>,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The progress bar used by this pipeline.
    progress: ProgressBar,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate paths to ignore back to the watcher.
    ignore_chan: Option<Sender<PathBuf>>,
    /// An optional binary name which will cause cargo & wasm-bindgen to process only the target
    /// binary.
    bin: Option<String>,
    /// An optional optimization setting that enables wasm-opt. Can be nothing, `0` (default), `1`,
    /// `2`, `3`, `4`, `s or `z`. Using `0` disables wasm-opt completely.
    wasm_opt: WasmOptLevel,
}

/// Different optimization levels that can be configured with `wasm-opt`.
#[derive(PartialEq, Eq)]
enum WasmOptLevel {
    /// Default optimization passes.
    Default,
    /// No optimization passes, skipping the wasp-opt step.
    Off,
    /// Run quick & useful optimizations. useful for iteration testing.
    One,
    /// Most optimizations, generally gets most performance.
    Two,
    /// Spend potentially a lot of time optimizing.
    Three,
    /// Also flatten the IR, which can take a lot more time and memory, but is useful on more nested
    /// / complex / less-optimized input.
    Four,
    /// Default optimizations, focus on code size.
    S,
    /// Default optimizations, super-focusing on code size.
    Z,
}

impl FromStr for WasmOptLevel {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "" => Self::Default,
            "0" => Self::Off,
            "1" => Self::One,
            "2" => Self::Two,
            "3" => Self::Three,
            "4" => Self::Four,
            "s" | "S" => Self::S,
            "z" | "Z" => Self::Z,
            _ => bail!("unknown wasm-opt level `{}`", s),
        })
    }
}

impl AsRef<str> for WasmOptLevel {
    fn as_ref(&self) -> &str {
        match self {
            Self::Default => "",
            Self::Off => "0",
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::S => "s",
            Self::Z => "z",
        }
    }
}

impl Default for WasmOptLevel {
    fn default() -> Self {
        // Current default is off until automatic download of wasm-opt is implemented.
        Self::Off
    }
}

impl RustApp {
    pub const TYPE_RUST_APP: &'static str = "rust";

    pub async fn new(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>, attrs: LinkAttrs, id: usize,
    ) -> Result<Self> {
        // Build the path to the target asset.
        let manifest_href = attrs
            .get(ATTR_HREF)
            .map(|attr| {
                let mut path = PathBuf::new();
                path.extend(attr.split('/'));
                if !path.is_absolute() {
                    path = html_dir.join(path);
                }
                if !path.ends_with("Cargo.toml") {
                    path = path.join("Cargo.toml");
                }
                path
            })
            .unwrap_or_else(|| html_dir.join("Cargo.toml"));
        let bin = attrs.get("data-bin").map(|val| val.to_string());
        let wasm_opt = attrs.get("wasm-opt").map(|val| val.parse()).transpose()?.unwrap_or_default();
        let manifest = CargoMetadata::new(&manifest_href).await?;
        let id = Some(id);

        Ok(Self {
            id,
            cfg,
            progress,
            manifest,
            ignore_chan,
            bin,
            wasm_opt,
        })
    }

    pub async fn new_default(
        cfg: Arc<RtcBuild>, progress: ProgressBar, html_dir: Arc<PathBuf>, ignore_chan: Option<Sender<PathBuf>>,
    ) -> Result<Self> {
        let path = html_dir.join("Cargo.toml");
        let manifest = CargoMetadata::new(&path).await?;
        Ok(Self {
            id: None,
            cfg,
            progress,
            manifest,
            ignore_chan,
            bin: None,
            wasm_opt: WasmOptLevel::default(),
        })
    }

    /// Spawn a new pipeline.
    pub fn spawn(self) -> JoinHandle<Result<TrunkLinkPipelineOutput>> {
        spawn(self.build())
    }

    async fn build(mut self) -> Result<TrunkLinkPipelineOutput> {
        let (wasm, hashed_name) = self.cargo_build().await?;
        let output = self.wasm_bindgen_build(wasm.as_ref(), &hashed_name).await?;
        self.wasm_opt_build(&output.wasm_output).await?;
        Ok(TrunkLinkPipelineOutput::RustApp(output))
    }

    async fn cargo_build(&mut self) -> Result<(PathBuf, String)> {
        self.progress.set_message(&format!("building {}", &self.manifest.package.name));

        // Spawn the cargo build process.
        let mut args = vec![
            "build",
            "--target=wasm32-unknown-unknown",
            "--manifest-path",
            &self.manifest.manifest_path,
        ];
        if self.cfg.release {
            args.push("--release");
        }
        if let Some(bin) = &self.bin {
            args.push("--bin");
            args.push(bin);
        }
        let build_output = Command::new("cargo")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning cargo build")?
            .output()
            .await
            .context("error during cargo build execution")?;
        ensure!(
            build_output.status.success(),
            "bad status returned from cargo build: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );

        // Canonicalize the cargo `target` dir, and send it over to the watcher to be ignored.
        // NB: if we attempt to do this pre-build, the canonicalization may fail.
        if let Some(chan) = &mut self.ignore_chan {
            let _ = chan.try_send(self.manifest.metadata.target_directory.clone());
        }

        // Perform a final cargo invocation on success to get artifact names.
        self.progress.set_message("fetching artifacts");
        args.push("--message-format=json");
        let artifacts_out = Command::new("cargo")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning cargo build artifacts task")?
            .output()
            .await
            .context("error getting cargo build artifacts info")?;
        ensure!(
            artifacts_out.status.success(),
            "bad status returned from cargo artifacts request: {}",
            String::from_utf8_lossy(&build_output.stderr)
        );

        // Stream over cargo messages to find the artifacts we are interested in.
        let reader = std::io::BufReader::new(artifacts_out.stdout.as_slice());
        let artifact = cargo_metadata::Message::parse_stream(reader)
            .filter_map(|msg| if let Ok(msg) = msg { Some(msg) } else { None })
            .fold(Ok(None), |acc, msg| match msg {
                cargo_metadata::Message::CompilerArtifact(art) if art.package_id == self.manifest.package.id => Ok(Some(art)),
                cargo_metadata::Message::BuildFinished(finished) if !finished.success => Err(anyhow!("error while fetching cargo artifact info")),
                _ => acc,
            })?
            .ok_or_else(|| anyhow!("cargo artifacts not found for target crate"))?;

        // Get a handle to the WASM output file.
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .ok_or_else(|| anyhow!("could not find WASM output after cargo build"))?;

        // Hash the built wasm app, then use that as the out-name param.
        self.progress.set_message("processing WASM");
        let wasm_bytes = fs::read(&wasm).await.context("error reading wasm file for hash generation")?;
        let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes));
        Ok((wasm, hashed_name))
    }

    async fn wasm_bindgen_build(&self, wasm: &Path, hashed_name: &str) -> Result<RustAppOutput> {
        self.progress.set_message("preparing for build");

        // Ensure our output dir is in place.
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let bindgen_out = self.manifest.metadata.target_directory.join("wasm-bindgen").join(mode_segment);
        fs::create_dir_all(bindgen_out.as_path())
            .await
            .context("error creating wasm-bindgen output dir")?;

        // Build up args for calling wasm-bindgen.
        let arg_out_path = format!("--out-dir={}", bindgen_out.display());
        let arg_out_name = format!("--out-name={}", &hashed_name);
        let target_wasm = wasm.to_string_lossy().to_string();
        let args = vec!["--target=web", &arg_out_path, &arg_out_name, "--no-typescript", &target_wasm];

        // Invoke wasm-bindgen.
        self.progress.set_message("calling wasm-bindgen");
        run_command("wasm-bindgen", &args).await?;

        self.progress.set_message("copying generated artifacts");

        // Copy the generated WASM & JS loader to the dist dir.
        let hashed_js_name = format!("{}.js", &hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &hashed_name);
        let js_loader_path = bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = self.cfg.staging_dist.join(&hashed_js_name);
        let wasm_path = bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = self.cfg.staging_dist.join(&hashed_wasm_name);
        fs::copy(js_loader_path, js_loader_path_dist)
            .await
            .context("error copying JS loader file to stage dir")?;
        fs::copy(wasm_path, wasm_path_dist)
            .await
            .context("error copying wasm file to stage dir")?;

        // Check for any snippets, and copy them over.
        let snippets_dir = bindgen_out.join(SNIPPETS_DIR);
        if path_exists(&snippets_dir).await? {
            copy_dir_recursive(bindgen_out.join(SNIPPETS_DIR), self.cfg.staging_dist.join(SNIPPETS_DIR))
                .await
                .context("error copying snippets dir to stage dir")?;
        }

        Ok(RustAppOutput {
            id: self.id,
            cfg: self.cfg.clone(),
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
        })
    }

    async fn wasm_opt_build(&self, hashed_name: &str) -> Result<()> {
        // Zero means no optimizations (valid arg for wasm-opt), so we can skip calling wasm-opt as
        // it wouldn't have any effect.
        if self.wasm_opt == WasmOptLevel::Off {
            return Ok(());
        }

        self.progress.set_message("calling wasm-opt");

        // Ensure our output dir is in place.
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let output = self.manifest.metadata.target_directory.join("wasm-opt").join(mode_segment);
        fs::create_dir_all(&output).await.context("error creating wasm-opt output dir")?;

        // Build up args for calling wasm-opt.
        let output = output.join(hashed_name);
        let arg_output = format!("--output={}", output.display());
        let arg_opt_level = format!("-O{}", self.wasm_opt.as_ref());
        let target_wasm = self.cfg.staging_dist.join(hashed_name).to_string_lossy().to_string();
        let args = vec![&arg_output, &arg_opt_level, &target_wasm];

        // Invoke wasm-opt.
        run_command("wasm-opt", &args).await?;

        // Copy the generated WASM file to the dist dir.
        self.progress.set_message("copying generated artifacts");
        fs::copy(output, self.cfg.staging_dist.join(hashed_name))
            .await
            .context("error copying wasm file to dist dir")?;

        // Delete old un-optimized WASM file from the staging dir.
        fs::remove_file(target_wasm)
            .await
            .context("error deleting un-optimized wasm file from dist dir")?;

        Ok(())
    }
}

/// The output of a cargo build pipeline.
pub struct RustAppOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: Option<usize>,
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}

impl RustAppOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        let script = format!(
            r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
            base = self.cfg.public_url,
            js = &self.js_output,
            wasm = &self.wasm_output,
        );
        match self.id {
            Some(id) => dom.select(&super::trunk_id_selector(id)).replace_with_html(script),
            None => dom.select("html head").append_html(script),
        }
        Ok(())
    }
}

/// Run a global command with the given arguments and make sure it completes successfully. If it
/// fails an error is returned.
async fn run_command(name: &str, args: &[impl AsRef<OsStr>]) -> Result<()> {
    let output = Command::new(name)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("error spawning {} call", name))?
        .output()
        .await
        .with_context(|| format!("error during {} call", name))?;
    ensure!(
        output.status.success(),
        "{} call returned a bad status {}",
        name,
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(())
}
