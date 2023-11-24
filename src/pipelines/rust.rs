//! Rust application pipeline.
use std::borrow::Cow;
use std::collections::HashMap;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, bail, ensure, Context, Result};
use base64::display::Base64Display;
use base64::engine::general_purpose::URL_SAFE;
use cargo_lock::Lockfile;
use cargo_metadata::camino::Utf8PathBuf;
use minify_js::TopLevelMode;
use nipper::Document;
use sha2::{Digest, Sha256, Sha384, Sha512};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::{Attrs, TrunkAssetPipelineOutput, ATTR_HREF, SNIPPETS_DIR};
use crate::common::{self, copy_dir_recursive, path_exists};
use crate::config::{CargoMetadata, ConfigOptsTools, CrossOrigin, Features, Integrity, RtcBuild};
use crate::tools::{self, Application};

/// A Rust application pipeline.
pub struct RustApp {
    /// The ID of this pipeline's source HTML element.
    id: Option<usize>,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// The configuration of the features passed to cargo.
    cargo_features: Features,
    /// Is this module main or a worker.
    app_type: RustAppType,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate paths to ignore back to the watcher.
    ignore_chan: Option<mpsc::Sender<PathBuf>>,
    /// An optional binary name which will cause cargo & wasm-bindgen to process only the target
    /// binary.
    bin: Option<String>,
    /// An option to instruct wasm-bindgen to preserve debug info in the final WASM output, even
    /// for `--release` mode.
    keep_debug: bool,
    /// An option to instruct wasm-bindgen to output Typescript bindings. Defaults to false
    typescript: bool,
    /// An option to instruct wasm-bindgen to not demangle Rust symbol names.
    no_demangle: bool,
    /// An option to instruct wasm-bindgen to enable reference types.
    reference_types: bool,
    /// An option to instruct wasm-bindgen to enable weak references.
    weak_refs: bool,
    /// An optional optimization setting that enables wasm-opt. Can be nothing, `0` (default), `1`,
    /// `2`, `3`, `4`, `s or `z`. Using `0` disables wasm-opt completely.
    wasm_opt: WasmOptLevel,
    /// Name for the module. Is binary name if given, otherwise it is the name of the cargo
    /// project.
    name: String,
    /// Whether to create a loader shim script
    loader_shim: bool,
    /// Cross origin setting for resources
    cross_origin: CrossOrigin,
    /// Subresource integrity setting
    integrity: Integrity,
}

/// Describes how the rust application is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RustAppType {
    /// Used as the main application.
    Main,
    /// Used as a web worker.
    Worker,
}

impl FromStr for RustAppType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "main" => Ok(RustAppType::Main),
            "worker" => Ok(RustAppType::Worker),
            _ => bail!(
                r#"unknown `data-type="{}"` value for <link data-trunk rel="rust" .../> attr; please ensure the value is lowercase and is a supported type"#,
                s
            ),
        }
    }
}

impl RustApp {
    pub const TYPE_RUST_APP: &'static str = "rust";

    pub async fn new(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
        attrs: Attrs,
        id: usize,
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
        let keep_debug = attrs.contains_key("data-keep-debug");
        let typescript = attrs.contains_key("data-typescript");
        let no_demangle = attrs.contains_key("data-no-demangle");
        let app_type = attrs
            .get("data-type")
            .map(|s| s.as_str())
            .unwrap_or("main")
            .parse()?;
        let reference_types = attrs.contains_key("data-reference-types");
        let weak_refs = attrs.contains_key("data-weak-refs");
        let wasm_opt = attrs
            .get("data-wasm-opt")
            .map(|val| val.parse())
            .transpose()?
            .unwrap_or_else(|| {
                if cfg.release {
                    Default::default()
                } else {
                    WasmOptLevel::Off
                }
            });
        let cross_origin = attrs
            .get("data-cross-origin")
            .map(|val| CrossOrigin::from_str(val))
            .transpose()?
            .unwrap_or_default();
        let integrity = attrs
            .get("data-integrity")
            .map(|val| Integrity::from_str(val))
            .transpose()?
            .unwrap_or_default();

        let manifest = CargoMetadata::new(&manifest_href).await?;
        let id = Some(id);
        let name = bin.clone().unwrap_or_else(|| manifest.package.name.clone());

        let data_features = attrs.get("data-cargo-features").map(|val| val.to_string());
        let data_all_features = attrs.get("data-cargo-all-features").is_some();
        let data_no_default_features = attrs.get("data-cargo-no-default-features").is_some();

        let loader_shim = attrs.get("data-loader-shim").is_some();
        if loader_shim {
            ensure!(
                app_type == RustAppType::Worker,
                "Loader shim has no effect when data-type is \"main\"!"
            );
        }

        // Highlander-rule: There can be only one (prohibits contradicting arguments):
        ensure!(
            !(data_all_features && (data_no_default_features || data_features.is_some())),
            "Cannot combine --all-features with --no-default-features and/or --features"
        );

        let cargo_features = if data_all_features {
            Features::All
        } else if data_no_default_features || data_features.is_some() {
            Features::Custom {
                features: data_features,
                no_default_features: data_no_default_features,
            }
        } else {
            // The features have not been overridden in the attributes so use the
            // features passed to cargo.
            cfg.cargo_features.clone()
        };

        Ok(Self {
            id,
            cfg,
            cargo_features,
            manifest,
            ignore_chan,
            bin,
            keep_debug,
            typescript,
            no_demangle,
            reference_types,
            weak_refs,
            wasm_opt,
            app_type,
            name,
            loader_shim,
            cross_origin,
            integrity,
        })
    }

    /// Create a new instance from reasonable defaults
    ///
    /// This will return `Ok(None)` in case no `Cargo.toml` was found. And fail in any case
    /// where no default could be evaluated.
    pub async fn new_default(
        cfg: Arc<RtcBuild>,
        html_dir: Arc<PathBuf>,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
    ) -> Result<Option<Self>> {
        let path = html_dir.join("Cargo.toml");

        if !tokio::fs::try_exists(&path).await? {
            // no Cargo.toml found, don't assume a project
            return Ok(None);
        }

        let manifest = CargoMetadata::new(&path).await?;
        let name = manifest.package.name.clone();

        Ok(Some(Self {
            id: None,
            cargo_features: cfg.cargo_features.clone(),
            cfg,
            manifest,
            ignore_chan,
            bin: None,
            keep_debug: false,
            typescript: false,
            no_demangle: false,
            reference_types: false,
            weak_refs: false,
            wasm_opt: WasmOptLevel::Off,
            app_type: RustAppType::Main,
            name,
            loader_shim: false,
            cross_origin: Default::default(),
            integrity: Default::default(),
        }))
    }

    /// Spawn a new pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput>> {
        tokio::spawn(self.build())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn build(mut self) -> Result<TrunkAssetPipelineOutput> {
        let (wasm, hashed_name, integrity) = self.cargo_build().await?;
        let output = self
            .wasm_bindgen_build(wasm.as_ref(), &hashed_name, integrity)
            .await?;
        self.wasm_opt_build(&output.wasm_output).await?;
        tracing::info!("rust build complete");
        Ok(TrunkAssetPipelineOutput::RustApp(output))
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn cargo_build(&mut self) -> Result<(PathBuf, String, IntegrityOutput)> {
        tracing::info!("building {}", &self.manifest.package.name);

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
        if self.cfg.offline {
            args.push("--offline");
        }
        if self.cfg.frozen {
            args.push("--frozen");
        }
        if self.cfg.locked {
            args.push("--locked");
        }
        if let Some(bin) = &self.bin {
            args.push("--bin");
            args.push(bin);
        }

        match &self.cargo_features {
            Features::All => args.push("--all-features"),
            Features::Custom {
                features,
                no_default_features,
            } => {
                if *no_default_features {
                    args.push("--no-default-features");
                }

                if let Some(cargo_features) = features {
                    args.push("--features");
                    args.push(cargo_features);
                }
            }
        }

        let build_res = common::run_command("cargo", Path::new("cargo"), &args)
            .await
            .context("error during cargo build execution");

        // Send cargo's target dir over to the watcher to be ignored. We must do this before
        // checking for errors, otherwise the dir will never be ignored. If we attempt to do
        // this pre-build, the canonicalization will fail and will not be ignored.
        if let Some(chan) = &mut self.ignore_chan {
            let _ = chan.try_send(
                self.manifest
                    .metadata
                    .target_directory
                    .clone()
                    .into_std_path_buf(),
            );
        }

        // Now propagate any errors which came from the cargo build.
        build_res?;

        // Perform a final cargo invocation on success to get artifact names.
        tracing::info!("fetching cargo artifacts");
        args.push("--message-format=json");
        let artifacts_out = Command::new("cargo")
            .args(args.as_slice())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("error spawning cargo build artifacts task")?
            .wait_with_output()
            .await
            .context("error getting cargo build artifacts info")?;
        if !artifacts_out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&artifacts_out.stderr));
            bail!("bad status returned from cargo artifacts request");
        }

        // Stream over cargo messages to find the artifacts we are interested in.
        let reader = std::io::BufReader::new(artifacts_out.stdout.as_slice());
        let mut bin_artifacts: Vec<cargo_metadata::Artifact> =
            cargo_metadata::Message::parse_stream(reader)
                .filter_map(|msg| msg.ok())
                .filter_map(|msg| match msg {
                    cargo_metadata::Message::CompilerArtifact(art)
                        if art.package_id == self.manifest.package.id
                            && (art.target.kind.contains(&"bin".to_string())
                                || art.target.kind.contains(&"cdylib".to_string())) =>
                    {
                        Some(Ok(art))
                    }
                    cargo_metadata::Message::BuildFinished(finished) if !finished.success => {
                        Some(Err(anyhow!("error while fetching cargo artifact info")))
                    }
                    _ => None,
                })
                .collect::<Result<_>>()?;
        // If there is already a `link data-trunk rel=rust` in index.html
        // then the --bin flag was passed to the cargo command
        // and it has built just a single binary
        if bin_artifacts.len() > 1 {
            bail!(
                "found more than one binary crate: {bin_names:?}, consider adding `<link \
                 data-trunk rel=\"rust\" data-bin={{bin}} />` to the index.html",
                bin_names = bin_artifacts
                    .iter()
                    .map(|a| &a.target.name)
                    .collect::<Vec<_>>()
            )
        }
        let Some(artifact) = bin_artifacts.pop() else {
            bail!("cargo artifacts not found for target crate")
        };

        // Get a handle to the WASM output file.
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .context("could not find WASM output after cargo build")?;

        // Hash the built wasm app, then use that as the out-name param.
        tracing::debug!("processing WASM for {}", self.name);
        let wasm_bytes = fs::read(&wasm)
            .await
            .context("error reading wasm file for hash generation")?;

        let mut integrity = IntegrityOutput::default();
        integrity.wasm = gen_digest(self.integrity, &wasm_bytes);

        // generate a hashed name
        let hashed_name = match (&self.integrity, self.cfg.filehash) {
            (_, false) => self.name.clone(),
            (Integrity::None, true) => format!("{}-{:x}", self.name, seahash::hash(&wasm_bytes)),
            (_, true) => {
                format!("{}-{}", self.name, hex::encode(&integrity.wasm.hash))
            }
        };

        Ok((wasm.into_std_path_buf(), hashed_name, integrity))
    }

    #[tracing::instrument(level = "trace", skip(self, wasm, hashed_name))]
    async fn wasm_bindgen_build(
        &self,
        wasm: &Path,
        hashed_name: &str,
        mut integrity: IntegrityOutput,
    ) -> Result<RustAppOutput> {
        // Skip the hashed file name for workers as their file name must be named at runtime.
        // Therefore, workers use the Cargo binary name for file naming.
        let hashed_name = match self.app_type {
            RustAppType::Main => hashed_name,
            RustAppType::Worker => &self.name,
        };

        let version = find_wasm_bindgen_version(&self.cfg.tools, &self.manifest);
        let wasm_bindgen = tools::get(
            Application::WasmBindgen,
            version.as_deref(),
            self.cfg.offline,
        )
        .await?;

        // Ensure our output dir is in place.
        let wasm_bindgen_name = Application::WasmBindgen.name();
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let bindgen_out = self
            .manifest
            .metadata
            .target_directory
            .join(wasm_bindgen_name)
            .join(mode_segment);
        fs::create_dir_all(bindgen_out.as_path())
            .await
            .context("error creating wasm-bindgen output dir")?;

        // Build up args for calling wasm-bindgen.
        let arg_out_path = format!("--out-dir={}", bindgen_out);
        let arg_out_name = format!("--out-name={}", &hashed_name);
        let target_wasm = wasm.to_string_lossy().to_string();
        let target_type = match self.app_type {
            RustAppType::Main => "--target=web",
            RustAppType::Worker => "--target=no-modules",
        };

        let mut args = vec![target_type, &arg_out_path, &arg_out_name, &target_wasm];
        if self.keep_debug {
            args.push("--keep-debug");
        }
        if self.no_demangle {
            args.push("--no-demangle");
        }
        if self.reference_types {
            args.push("--reference-types");
        }
        if self.weak_refs {
            args.push("--weak-refs");
        }

        if !self.typescript {
            args.push("--no-typescript");
        }

        // Invoke wasm-bindgen.
        tracing::info!("calling wasm-bindgen for {}", self.name);
        common::run_command(wasm_bindgen_name, &wasm_bindgen, &args)
            .await
            .map_err(|err| check_target_not_found_err(err, wasm_bindgen_name))?;

        // Copy the generated WASM & JS loader to the dist dir.
        tracing::debug!("copying generated wasm-bindgen artifacts");
        let hashed_js_name = format!("{}.js", &hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &hashed_name);
        let hashed_ts_name = format!("{}.d.ts", &hashed_name);
        let js_loader_path = bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = self.cfg.staging_dist.join(&hashed_js_name);
        let wasm_path = bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = self.cfg.staging_dist.join(&hashed_wasm_name);
        let hashed_loader_name = self
            .loader_shim
            .then(|| format!("{}_loader.js", &hashed_name));
        let loader_shim_path = hashed_loader_name
            .as_ref()
            .map(|m| self.cfg.staging_dist.join(m));

        tracing::debug!(
            "copying {js_loader_path} to {}",
            js_loader_path_dist.to_string_lossy()
        );
        self.copy_or_minify_js(js_loader_path, &js_loader_path_dist)
            .await
            .context("error minifying or copying JS loader file to stage dir")?;

        tracing::debug!(
            "copying {wasm_path} to {}",
            wasm_path_dist.to_string_lossy()
        );

        fs::copy(wasm_path, wasm_path_dist)
            .await
            .context("error copying wasm file to stage dir")?;

        if self.typescript {
            let ts_path = bindgen_out.join(&hashed_ts_name);
            let ts_path_dist = self.cfg.staging_dist.join(&hashed_ts_name);

            tracing::debug!("copying {ts_path} to {}", ts_path_dist.to_string_lossy());
            fs::copy(ts_path, ts_path_dist)
                .await
                .context("error copying TS files to stage dir")?;
        }

        if let Some(ref m) = loader_shim_path {
            tracing::debug!("creating {}", m.to_string_lossy());
            let mut loader_f = fs::File::create(m)
                .await
                .context("error creating loader shim script")?;

            loader_f
                .write_all(
                    format!(
                        r#"importScripts("./{}");wasm_bindgen("./{}");"#,
                        hashed_js_name, hashed_wasm_name
                    )
                    .as_bytes(),
                )
                .await
                .context("error writing loader shim script")?;
            loader_f
                .flush()
                .await
                .context("error writing loader shim script")?;
        }

        let ts_output = if self.typescript {
            Some(hashed_ts_name)
        } else {
            None
        };

        // Check for any snippets, and copy them over.
        let snippets_dir_src = bindgen_out.join(SNIPPETS_DIR);
        if path_exists(&snippets_dir_src).await? {
            let snippets_dir_dest = self.cfg.staging_dist.join(SNIPPETS_DIR);
            tracing::debug!(
                "recursively copying from '{snippets_dir_src}' to '{}'",
                snippets_dir_dest.to_string_lossy()
            );
            copy_dir_recursive(snippets_dir_src, snippets_dir_dest)
                .await
                .context("error copying snippets dir to stage dir")?;
        }

        integrity.js = gen_digest(self.integrity, &std::fs::read(js_loader_path_dist)?);

        Ok(RustAppOutput {
            id: self.id,
            cfg: self.cfg.clone(),
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
            ts_output,
            loader_shim_output: hashed_loader_name,
            type_: self.app_type,
            cross_origin: self.cross_origin,
            integrity,
        })
    }

    async fn copy_or_minify_js(
        &self,
        origin_path: Utf8PathBuf,
        destination_path: &Path,
    ) -> Result<()> {
        let bytes = fs::read(origin_path)
            .await
            .context("error reading JS loader file")?;

        let write_bytes = match self.cfg.release {
            true => {
                let mut output: Vec<u8> = vec![];
                let bytes_clone = bytes.clone();
                let session = minify_js::Session::new();
                let res = minify_js::minify(&session, TopLevelMode::Module, &bytes, &mut output);
                if res.is_err() {
                    output = bytes_clone;
                }
                output
            }
            false => bytes,
        };

        fs::write(destination_path, write_bytes)
            .await
            .context("error writing JS loader file to stage dir")?;
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self, hashed_name))]
    async fn wasm_opt_build(&self, hashed_name: &str) -> Result<()> {
        // If not in release mode, we skip calling wasm-opt.
        if !self.cfg.release {
            return Ok(());
        }

        // If opt level is off, we skip calling wasm-opt as it wouldn't have any effect.
        if self.wasm_opt == WasmOptLevel::Off {
            return Ok(());
        }

        let version = self.cfg.tools.wasm_opt.as_deref();
        let wasm_opt = tools::get(Application::WasmOpt, version, self.cfg.offline).await?;

        // Ensure our output dir is in place.
        let wasm_opt_name = Application::WasmOpt.name();
        let mode_segment = if self.cfg.release { "release" } else { "debug" };
        let output = self
            .manifest
            .metadata
            .target_directory
            .join(wasm_opt_name)
            .join(mode_segment);
        fs::create_dir_all(&output)
            .await
            .context("error creating wasm-opt output dir")?;

        // Build up args for calling wasm-opt.
        let output = output.join(hashed_name);
        let arg_output = format!("--output={}", output);
        let arg_opt_level = format!("-O{}", self.wasm_opt.as_ref());
        let target_wasm = self
            .cfg
            .staging_dist
            .join(hashed_name)
            .to_string_lossy()
            .to_string();
        let mut args: Vec<&str> = vec![&arg_output, &arg_opt_level, &target_wasm];

        if self.reference_types {
            args.push("--enable-reference-types");
        }

        // Invoke wasm-opt.
        tracing::info!("calling wasm-opt");
        common::run_command(wasm_opt_name, &wasm_opt, &args)
            .await
            .map_err(|err| check_target_not_found_err(err, wasm_opt_name))?;

        // Copy the generated WASM file to the dist dir.
        tracing::debug!("copying generated wasm-opt artifacts");
        fs::copy(output, self.cfg.staging_dist.join(hashed_name))
            .await
            .context("error copying wasm file to dist dir")?;

        Ok(())
    }
}

/// Find the appropriate version of `wasm-bindgen` to use. The version can be found in 3 different
/// location in order:
/// - Defined in the `Trunk.toml` as highest priority.
/// - Located in the `Cargo.lock` if it exists. This is mostly the case as we run `cargo build`
///   before even calling this function.
/// - Located in the `Cargo.toml` as direct dependency of the project.
fn find_wasm_bindgen_version<'a>(
    cfg: &'a ConfigOptsTools,
    manifest: &CargoMetadata,
) -> Option<Cow<'a, str>> {
    let find_lock = || -> Option<Cow<'_, str>> {
        let lock_path = Path::new(&manifest.manifest_path)
            .parent()?
            .join("Cargo.lock");
        let lockfile = Lockfile::load(lock_path).ok()?;
        let name = "wasm-bindgen".parse().ok()?;

        lockfile
            .packages
            .into_iter()
            .find(|p| p.name == name)
            .map(|p| Cow::from(p.version.to_string()))
    };

    let find_manifest = || -> Option<Cow<'_, str>> {
        manifest
            .metadata
            .packages
            .iter()
            .find(|p| p.name == "wasm-bindgen")
            .map(|p| Cow::from(p.version.to_string()))
    };

    cfg.wasm_bindgen
        .as_deref()
        .map(Cow::from)
        .or_else(find_lock)
        .or_else(find_manifest)
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
    /// The filename of the generated .ts file written to the dist dir.
    pub ts_output: Option<String>,
    /// The filename of the generated loader shim script for web workers written to the dist dir.
    pub loader_shim_output: Option<String>,
    /// Is this module main or a worker.
    pub type_: RustAppType,
    /// The cross-origin setting for loading the resources
    pub cross_origin: CrossOrigin,
    /// The integrity and digest of the output, ignored in case of [`Integrity::None`]
    pub integrity: IntegrityOutput,
}

pub fn pattern_evaluate(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in params.iter() {
        let pattern = format!("{{{}}}", k.as_str());
        if let Some(file_path) = v.strip_prefix('@') {
            if let Ok(contents) = std::fs::read_to_string(file_path) {
                result = str::replace(result.as_str(), &pattern, contents.as_str());
            }
        } else {
            result = str::replace(result.as_str(), &pattern, v);
        }
    }
    result
}

impl RustAppOutput {
    pub async fn finalize(self, dom: &mut Document) -> Result<()> {
        if self.type_ == RustAppType::Worker {
            // Skip the script tag and preload links for workers, and remove the link tag only.
            // Workers are initialized and managed by the app itself at runtime.
            if let Some(id) = self.id {
                dom.select(&super::trunk_id_selector(id)).remove();
            }
            return Ok(());
        }

        if !self.cfg.inject_scripts {
            // Configuration directed we do not inject any scripts.
            return Ok(());
        }

        let (base, js, wasm, head, body) = (
            &self.cfg.public_url,
            &self.js_output,
            &self.wasm_output,
            "html head",
            "html body",
        );
        let (pattern_script, pattern_preload) =
            (&self.cfg.pattern_script, &self.cfg.pattern_preload);
        let mut params: HashMap<String, String> = match &self.cfg.pattern_params {
            Some(x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), base.clone());
        params.insert("js".to_owned(), js.clone());
        params.insert("wasm".to_owned(), wasm.clone());
        params.insert("crossorigin".to_owned(), self.cross_origin.to_string());

        let preload = match pattern_preload {
            Some(pattern) => pattern_evaluate(pattern, &params),
            None => {
                format!(
                    r#"
<link rel="preload" href="{base}{wasm}" as="fetch" type="application/wasm" crossorigin={cross_origin}{wasm_integrity}>
<link rel="modulepreload" href="{base}{js}" crossorigin={cross_origin}{js_integrity}>"#,
                    cross_origin = self.cross_origin,
                    wasm_integrity = self.integrity.wasm.make_attribute(),
                    js_integrity = self.integrity.js.make_attribute(),
                )
            }
        };
        dom.select(head).append_html(preload);

        let script = match pattern_script {
            Some(pattern) => pattern_evaluate(pattern, &params),
            None => {
                format!(
                    r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
                )
            }
        };
        match self.id {
            Some(id) => dom
                .select(&super::trunk_id_selector(id))
                .replace_with_html(script),
            None => dom.select(body).append_html(script),
        }
        Ok(())
    }
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
    /// Also flatten the IR, which can take a lot more time and memory, but is useful on more
    /// nested / complex / less-optimized input.
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
        Self::Default
    }
}

/// Handle invocation errors indicating that the target binary was not found, simply wrapping the
/// error in additional context stating more clearly that the target was not found.
fn check_target_not_found_err(err: anyhow::Error, target: &str) -> anyhow::Error {
    let io_err: &std::io::Error = match err.downcast_ref() {
        Some(io_err) => io_err,
        None => return err,
    };
    match io_err.kind() {
        std::io::ErrorKind::NotFound => err.context(format!("{} not found", target)),
        _ => err,
    }
}

/// Integrity of outputs
#[derive(Debug, Default)]
pub struct IntegrityOutput {
    pub wasm: OutputDigest,
    pub js: OutputDigest,
}

/// The digest of the output
#[derive(Debug)]
pub struct OutputDigest {
    pub integrity: Integrity,
    pub hash: Vec<u8>,
}

impl Default for OutputDigest {
    fn default() -> Self {
        Self {
            integrity: Integrity::None,
            hash: vec![],
        }
    }
}

impl OutputDigest {
    pub fn make_attribute(&self) -> String {
        match self.integrity {
            Integrity::None => String::default(),
            integrity => {
                // format of an attribute, including the leading space
                format!(
                    r#" integrity="{integrity}-{hash}""#,
                    hash = Base64Display::new(&self.hash, &URL_SAFE)
                )
            }
        }
    }
}

fn gen_digest(integrity: Integrity, data: &[u8]) -> OutputDigest {
    let hash = match integrity {
        Integrity::None => vec![],
        Integrity::Sha256 => Vec::from_iter(Sha256::digest(data)),
        Integrity::Sha384 => Vec::from_iter(Sha384::digest(data)),
        Integrity::Sha512 => Vec::from_iter(Sha512::digest(data)),
    };

    OutputDigest { integrity, hash }
}
