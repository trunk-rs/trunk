//! Build system & asset pipelines.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use async_process::{Command, Stdio};
use async_std::task::{spawn, spawn_blocking, JoinHandle};
use async_std::{fs, path};
use cargo_metadata::{Metadata, MetadataCommand, Package};
use console::Emoji;
use futures::stream::{FuturesUnordered, StreamExt};
use indicatif::ProgressBar;
use nipper::Document;

use crate::config::RtcBuild;

const TRUNK_ID: &str = "__trunk-id";
const HREF_ATTR: &str = "href";
const SNIPPETS_DIR: &str = "snippets";

/// A system used for building a Rust WASM app & bundling its assets.
///
/// This unit of data should be used throughout the system for driving build processes and
/// bundling tasks. Different CLI commands which need to trigger builds in some way should
/// be able to gather the needed data to create an instance of this struct, and then the vairous
/// build routines can be cleanly abstracted away form any specific CLI endpoints.
pub struct BuildSystem {
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The path to the source HTML document from which the output `index.html` will be built.
    target_html_path: Arc<PathBuf>,
    /// The parent directory of `target_html_path`.
    target_html_dir: Arc<PathBuf>,

    /// The output dir of the wasm-bindgen execution.
    bindgen_out: Arc<PathBuf>,
    /// The path to the app's output WASM.
    app_target_wasm: Arc<PathBuf>,

    /// A stream of asset pipelines.
    pipelines: FuturesUnordered<JoinHandle<Result<AssetPipelineOutput>>>,

    /// The object used for writing data to stdout, stderr & controlling the progress spinner.
    progress: ProgressBar,
}

impl BuildSystem {
    /// Create a new instance from the raw components.
    ///
    /// Reducing the number of assumptions here should help us to stay flexible when adding new
    /// commands, rafctoring and the like.
    pub async fn new(cfg: Arc<RtcBuild>) -> Result<Self> {
        let mode_segment = if cfg.release { "release" } else { "debug" };
        let app_target_wasm = cfg
            .manifest
            .metadata
            .target_directory
            .join("wasm32-unknown-unknown")
            .join(mode_segment)
            .join(format!("{}.wasm", &cfg.manifest.name));
        let bindgen_out = cfg.manifest.metadata.target_directory.join("wasm-bindgen").join(mode_segment);
        let target_html_path = cfg
            .target
            .canonicalize()
            .map_err(|err| anyhow!("failed to get canonical path of target HTML file: {}", err))?;
        let target_html_dir = target_html_path
            .parent()
            .ok_or_else(|| anyhow!("failed to determine parent dir of target HTML file"))?
            .to_owned();
        Ok(Self {
            cfg,
            target_html_path: Arc::new(target_html_path),
            target_html_dir: Arc::new(target_html_dir),
            bindgen_out: Arc::new(bindgen_out),
            app_target_wasm: Arc::new(app_target_wasm),
            pipelines: FuturesUnordered::new(),
            progress: ProgressBar::new_spinner(),
        })
    }

    /// Get a handle to the progress / terminal system.
    pub fn get_progress_handle(&self) -> ProgressBar {
        self.progress.clone()
    }

    /// Build the application described in the given build data.
    pub async fn build(&mut self) -> Result<()> {
        self.progress.reset();
        self.progress.enable_steady_tick(100);
        let res = self.do_build().await;
        self.progress.disable_steady_tick();
        if let Err(err) = res {
            self.progress
                .finish_with_message(&format!("{}build finished with errors", Emoji("❌ ", "")));
            return Err(err);
        }
        self.progress
            .finish_with_message(&format!("{}build completed successfully", Emoji("✅ ", "")));
        Ok(())
    }

    async fn do_build(&mut self) -> Result<()> {
        // Update the contents of the source HTML.
        let target_html_raw = fs::read_to_string(self.target_html_path.as_ref()).await?;
        let mut target_html = Document::from(&target_html_raw);

        // Spawn cargo build. It will run concurrently without polling.
        // When ready, await to get the final output.
        let cargo_build_handle = self.spawn_cargo_build();

        // Ensure output directories are in place.
        fs::create_dir_all(self.cfg.dist.as_path()).await?;
        fs::create_dir_all(self.bindgen_out.as_path()).await?;

        // Begin processing source HTML assets. Asset pipeline handles are pushed to `self.pipelines`.
        self.spawn_asset_pipelines(&mut target_html).await?;

        // Spawn the wasm-bindgen call to perform that last leg of application setup.
        let bindgen_file_name = cargo_build_handle.await?; // We need the `cargo build` output first.
        let wasm_bindgen_output = self.spawn_wasm_bindgen_build(bindgen_file_name).await?;

        // Finalize asset pipelines.
        self.finalize_asset_pipelines(&mut target_html).await;
        self.insert_wasm_module(&wasm_bindgen_output, &mut target_html);

        // Assemble a new output index.html file.
        let output_html = target_html.html(); // TODO: prettify this output.
        fs::write(format!("{}/index.html", self.cfg.dist.display()), output_html.as_bytes()).await?;
        Ok(())
    }

    /// Finalize asset pipelines & prep the DOM for final output.
    async fn finalize_asset_pipelines(&mut self, target_html: &mut Document) {
        while let Some(asset_res) = self.pipelines.next().await {
            // Unpack the asset pipeline result.
            let asset = match asset_res {
                Ok(asset) => asset,
                Err(err) => {
                    self.progress.println(format!("{}", err));
                    continue;
                }
            };
            // Update the DOM based on asset output.
            let mut node = target_html.select(&format!("[{}={}]", TRUNK_ID, &asset.id));
            if asset.remove {
                node.remove();
            } else {
                node.remove_attr(TRUNK_ID);
                node.remove_attr(HREF_ATTR);
                node.set_attr(HREF_ATTR, &format!("{}{}", &self.cfg.public_url, &asset.file_name));
            }
        }
        // Remove any additional trunk IDs from the DOM.
        target_html.select(&format!("[{}]", TRUNK_ID)).remove_attr(TRUNK_ID);
    }

    /// Insert the finalized WASM into the output HTML.
    fn insert_wasm_module(&mut self, wasm: &WasmBindgenOutput, target_html: &mut Document) {
        let script = format!(
            r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
            base = self.cfg.public_url,
            js = &wasm.js_output,
            wasm = &wasm.wasm_output,
        );
        target_html.select("head").append_html(script);
    }

    /// Spawn a cargo build process.
    ///
    /// The output is a file name "stem" which includes a hash of the build WASM object. This
    /// value is intended to be fed to wasm-bindgen to be used as the output file name.
    fn spawn_cargo_build(&self) -> JoinHandle<Result<String>> {
        // Start the cargo build in the background.
        let mut args = vec!["build", "--target=wasm32-unknown-unknown"];
        if self.cfg.release {
            args.push("--release");
        }
        self.progress.set_message(&format!(
            "{}starting cargo build on {}",
            Emoji("📦 ", ""),
            &self.cfg.manifest.package.name
        ));
        let app_target_wasm = self.app_target_wasm.clone();
        spawn(async move {
            // Spawn the cargo build process.
            let build_result = Command::new("cargo")
                .args(args.as_slice())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| anyhow!("error spawning cargo build: {}", err))?
                .output()
                .await;
            // Handle build results.
            match build_result {
                Ok(output) => {
                    if !output.status.success() {
                        return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr)));
                    }
                }
                Err(err) => return Err(anyhow!("error during cargo build: {}", err)),
            }
            // Hash the built wasm app, then use that as the out-name param.
            let wasm_bytes = fs::read(app_target_wasm.as_ref()).await?;
            let hashed_name = format!("index-{:x}", seahash::hash(&wasm_bytes));
            Ok(hashed_name)
        })
    }

    /// Spawn the wasm-bindgen build process.
    fn spawn_wasm_bindgen_build(&self, file_name: String) -> JoinHandle<Result<WasmBindgenOutput>> {
        let (dist, bindgen_out, app_target_wasm) = (self.cfg.dist.clone(), self.bindgen_out.clone(), self.app_target_wasm.clone());
        let (snippets_dir_from, snippets_dir_to) = (self.bindgen_out.join(SNIPPETS_DIR), self.cfg.dist.join(SNIPPETS_DIR));

        self.progress.set_message(&format!("{}starting wasm-bindgen build", Emoji("📦 ", "")));
        spawn(async move {
            let arg_out_path = format!("--out-dir={}", bindgen_out.display());
            let arg_out_name = format!("--out-name={}", &file_name);
            let target_wasm = app_target_wasm.to_string_lossy().to_string();

            // Spawn the wasm-bindgen process.
            let args = vec!["--target=web", &arg_out_path, &arg_out_name, "--no-typescript", &target_wasm];
            let build_result = Command::new("wasm-bindgen")
                .args(args.as_slice())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| anyhow!("error spawning wasm-bindgen build: {}", err))?
                .output()
                .await;

            // Handle build results.
            match build_result {
                Ok(output) => {
                    if !output.status.success() {
                        return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr)));
                    }
                }
                Err(err) => return Err(anyhow!("error during wasm-bindgen build: {}", err)),
            }

            // Copy the generated WASM & JS loader to the dist dir, and generate the needed body
            // for the output HTML.
            let hashed_js_name = format!("{}.js", &file_name);
            let hashed_wasm_name = format!("{}_bg.wasm", &file_name);
            let js_loader_path = bindgen_out.join(&hashed_js_name);
            let js_loader_path_dist = dist.join(&hashed_js_name);
            let wasm_path = bindgen_out.join(&hashed_wasm_name);
            let wasm_path_dist = dist.join(&hashed_wasm_name);
            fs::copy(js_loader_path, js_loader_path_dist).await?;
            fs::copy(wasm_path, wasm_path_dist).await?;

            // Check for any snippets, and copy them over.
            Self::copy_dir_recursive(snippets_dir_from, snippets_dir_to)
                .await
                .with_context(|| "error copying snippets dir")?;

            Ok(WasmBindgenOutput {
                js_output: hashed_js_name,
                wasm_output: hashed_wasm_name,
            })
        })
    }

    /// A utility function to recursively copy a directory.
    async fn copy_dir_recursive(from_dir: PathBuf, to_dir: PathBuf) -> Result<()> {
        if !path::PathBuf::from(&from_dir).exists().await {
            return Ok(());
        }
        spawn_blocking(move || {
            let opts = fs_extra::dir::CopyOptions {
                overwrite: true,
                content_only: true,
                ..Default::default()
            };
            Ok(fs_extra::dir::copy(from_dir, to_dir, &opts)?)
        })
        .await
        .map(|_| ())
    }

    /// Spawn asset building/bundling pipelines.
    ///
    /// Assets are given an ID which corresponds to an ID added to the DOM. Once the processing
    /// for the asset is finished, it will be able to update the DOM correctly based on its own
    /// ID. All of these trunk specific IDs will be removed from the DOM before it is written.
    async fn spawn_asset_pipelines(&mut self, target_html: &mut Document) -> Result<()> {
        self.progress.set_message(&format!("{}spawning asset pipelines", Emoji("📦 ", "")));

        // Accumulate assets declared in HTML head section links for processing.
        let asset_links = target_html
            .select(r#"html head link"#)
            .iter()
            .filter_map(|node| {
                // Be sure our link has an href to process, else skip.
                let href = match node.attr("href") {
                    Some(href) => href,
                    None => return None,
                };
                Some((node, href))
            })
            .enumerate();

        // Update the DOM for each extracted asset as long as it is a valid FS path.
        let mut assets = vec![];
        for (idx, (mut node, href)) in asset_links {
            // Take the path to referenced resource, if it is a valid asset, then we continue.
            let path = self.target_html_dir.join(href.as_ref());
            let rel = node.attr_or("rel", "").to_string().to_lowercase();
            let id = format!("link-{}", idx);
            let asset = match AssetFile::new(path, AssetType::Link { rel }, id, &self.progress).await {
                Ok(asset) => asset,
                Err(_) => continue,
            };
            // Update the DOM with an ID for async processing.
            node.set_attr(TRUNK_ID, &asset.id);
            assets.push(asset);
        }

        // Route assets over to the appropriate pipeline handler.
        for asset in assets {
            self.spawn_asset_bundle(asset);
        }
        Ok(())
    }

    /// Spawn an build pipeline for the given asset based on its file extension.
    fn spawn_asset_bundle(&mut self, asset: AssetFile) {
        let handle = match &asset.atype {
            AssetType::Link { rel } => match rel.as_ref() {
                "stylesheet" => match asset.ext.as_ref() {
                    "scss" | "sass" => self.spawn_sass_pipeline(asset),
                    "css" => self.spawn_copy_pipeline(asset, true, false),
                    _ => self.spawn_copy_pipeline(asset, false, false),
                },
                "icon" => self.spawn_copy_pipeline(asset, true, false),
                "trunk-dist" => self.spawn_copy_pipeline(asset, false, true),
                _ => return,
            },
        };
        // Push the handle into a queue for async collection.
        self.pipelines.push(handle);
    }

    /// Spawn a concurrent build pipeline for a SASS/SCSS asset.
    fn spawn_sass_pipeline(&mut self, asset: AssetFile) -> JoinHandle<Result<AssetPipelineOutput>> {
        let (dist, release, progress) = (self.cfg.dist.clone(), self.cfg.release, self.progress.clone());
        spawn(async move {
            // Compile the target SASS/SCSS file.
            let path_str = asset.path.to_string_lossy().to_string();
            let mut opts = sass_rs::Options::default();
            if release {
                opts.output_style = sass_rs::OutputStyle::Compressed;
            }
            let css = spawn_blocking(move || match sass_rs::compile_file(&path_str, opts) {
                Ok(css) => Ok(css),
                Err(err) => {
                    progress.println(err);
                    Err(anyhow!("error compiling sass for {}", &path_str))
                }
            })
            .await?;
            // Hash the contents to generate a file name, and then write the contents to the dist dir.
            let hash = seahash::hash(css.as_bytes());
            let file_name = asset.file_stem.to_string_lossy();
            let out_file_name = format!("{}-{:x}.css", file_name, hash);
            let out_file = dist.join(&out_file_name);
            fs::write(out_file, css).await?;
            Ok(AssetPipelineOutput {
                id: asset.id,
                file_name: out_file_name,
                remove: false,
            })
        })
    }

    /// Spawn a concurrent build pipeline which simply copies the source to the destination, unchanged.
    fn spawn_copy_pipeline(&mut self, asset: AssetFile, hash: bool, remove: bool) -> JoinHandle<Result<AssetPipelineOutput>> {
        let dist = self.cfg.dist.clone();
        spawn(async move {
            let bytes = fs::read(&asset.path).await?;
            let new_file_name = if hash {
                let hash = seahash::hash(bytes.as_ref());
                let orig_file_name = asset.file_stem.to_string_lossy();
                format!("{}-{:x}.{}", orig_file_name, hash, &asset.ext)
            } else {
                asset.file_name.to_string_lossy().to_string()
            };

            let out_file_name = dist.join(&new_file_name);
            fs::write(out_file_name, bytes).await?;

            Ok(AssetPipelineOutput {
                id: asset.id,
                file_name: new_file_name,
                remove,
            })
        })
    }
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

/// An asset type descriptor extracted from the source HTML.
enum AssetType {
    Link {
        /// The `rel` attribute of the HTML link.
        rel: String,
    },
}

/// An asset file to be processed by some build pipeline.
struct AssetFile {
    /// The canonicalized path to the target file.
    pub path: PathBuf,
    /// The name of the file itself.
    pub file_name: OsString,
    /// The file stem of the asset file.
    pub file_stem: OsString,
    /// The extension of the file.
    pub ext: String,
    /// The asset's type.
    pub atype: AssetType,
    /// The ID which this asset should use.
    pub id: String,
}

impl AssetFile {
    /// Create a new instance.
    ///
    /// The given path will be validated to ensure the following:
    /// - that the full canonicalized path points to a file on the FS.
    /// - that the file has a filename.
    /// - that the file has an extension.
    ///
    /// Any errors returned from this constructor indicate that one of these invariants was not
    /// upheld.
    pub async fn new(path: PathBuf, atype: AssetType, id: String, progress: &ProgressBar) -> Result<Self> {
        // Take the path to referenced resource, if it is actually an FS path, then we continue.
        let path = match fs::canonicalize(&path).await {
            Ok(path) => path,
            Err(_) => {
                if !path.to_string_lossy().contains("://") {
                    progress.println(format!("{}skipping invalid path: {}", Emoji("︎🚫 ", ""), path.to_string_lossy()));
                }
                return Err(anyhow!("skipping asset which is not a valid path"));
            }
        };
        ensure!(path.is_file().await, "target file does not exist on the FS");
        let file_name = match path.file_name() {
            Some(file_name) => file_name.to_owned(),
            None => bail!("asset has no file name"),
        };
        let file_stem = match path.file_stem() {
            Some(file_stem) => file_stem.to_owned(),
            None => bail!("asset has no file name stem"),
        };
        let ext = match path.extension() {
            Some(ext) => ext.to_string_lossy().to_lowercase(),
            None => bail!("asset has no file extension"),
        };
        Ok(Self {
            path: path.into(),
            file_name,
            file_stem,
            ext,
            atype,
            id,
        })
    }
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

/// The output of an asset pipeline.
pub struct AssetPipelineOutput {
    /// The ID of the asset pipeline.
    pub id: String,
    /// The file name of the output file written to the dist dir (not a full path).
    pub file_name: String,
    /// A bool indicating if the HTML node associated with this pipeline should be removed.
    pub remove: bool,
}

/// The output of the wasm-bindgen process.
struct WasmBindgenOutput {
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
}

//////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////

/// A wrapper around the cargo project's metadata.
#[derive(Clone, Debug)]
pub struct CargoMetadata {
    /// The metadata parsed from the cargo project.
    pub metadata: Metadata,
    /// The metadata package info on this package.
    pub package: Package,
    /// The name of the cargo project's build output file after `s/-/_/` replacement.
    pub name: String,
}

impl CargoMetadata {
    /// Get the project's cargo metadata of the CWD, or of the project specified by the given manifest path.
    pub async fn new(manifest: &Option<PathBuf>) -> Result<Self> {
        // Fetch the cargo project's metadata.
        let mut cmd = MetadataCommand::new();
        if let Some(manifest) = manifest.as_ref() {
            cmd.manifest_path(manifest);
        }
        let metadata = spawn_blocking(move || cmd.exec()).await?;

        // Get a handle to this project's package info.
        let resolve = match metadata.resolve.as_ref() {
            Some(resolve) => resolve,
            None => bail!("missing package info from cargo project metadata"),
        };
        let pkgid = match resolve.root.as_ref() {
            Some(pkgid) => pkgid,
            None => bail!("package id missing while processing cargo metadata"),
        };
        let package = match metadata.packages.iter().find(|pkg| &pkg.id == pkgid) {
            Some(package) => package.clone(),
            None => bail!("error finding package info in cargo metadata"),
        };
        let name = package.name.replace("-", "_");

        Ok(Self { metadata, package, name })
    }
}
