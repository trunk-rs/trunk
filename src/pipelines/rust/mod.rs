//! Rust application pipeline.

mod output;
mod sri;
mod wasm_bindgen;
mod wasm_opt;

pub use output::RustAppOutput;

use super::{data_target_path, Attrs, TrunkAssetPipelineOutput, ATTR_HREF, SNIPPETS_DIR};
use crate::{
    common::{
        self, apply_data_target_path, check_target_not_found_err, copy_dir_recursive, path_exists,
        path_to_href, target_path,
    },
    config::{
        rt::{Features, RtcBuild},
        types::CrossOrigin,
        CargoMetadata,
    },
    pipelines::rust::sri::{SriBuilder, SriOptions, SriType},
    processing::{integrity::IntegrityType, minify::minify_js},
    tools::{self, Application, ToolInformation},
};
use anyhow::{anyhow, bail, ensure, Context, Result};
use cargo_metadata::{Artifact, TargetKind};
use minify_js::TopLevelMode;
use seahash::SeaHasher;
use std::{
    collections::HashSet,
    hash::Hasher,
    path::{Path, PathBuf},
    process::Stdio,
    str::FromStr,
    sync::Arc,
};
use tokio::{fs, io::AsyncWriteExt, process::Command, sync::mpsc, task::JoinHandle};
use tracing::log;
use wasm_bindgen::{find_wasm_bindgen_version, WasmBindgenFeatures, WasmBindgenTarget};
use wasm_opt::WasmOptLevel;

/// A Rust application pipeline.
pub struct RustApp {
    /// The ID of this pipeline's source HTML element.
    id: Option<usize>,
    /// Runtime config.
    cfg: Arc<RtcBuild>,
    /// Skip building
    skip_build: bool,
    /// Cargo profile to use
    cargo_profile: Option<String>,
    /// The configuration of the features passed to cargo.
    cargo_features: Features,
    /// Is this module main or a worker?
    app_type: RustAppType,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,
    /// An optional channel to be used to communicate paths to ignore back to the watcher.
    ignore_chan: Option<mpsc::Sender<PathBuf>>,
    /// An optional binary name which will cause cargo & wasm-bindgen to process only the target
    /// binary.
    bin: Option<String>,
    /// An optional filter for finding the target artifact.
    target_name: Option<String>,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
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
    /// An optional optimization command line params to wasm-opt if it is enabled.
    wasm_opt_params: Vec<String>,
    /// The value of the `--target` flag for wasm-bindgen.
    wasm_bindgen_target: WasmBindgenTarget,
    /// Name for the module. Is binary name if given, otherwise it is the name of the cargo
    /// project.
    name: String,
    /// Whether to create a loader shim script
    loader_shim: bool,
    /// Cross-origin setting for resources
    cross_origin: CrossOrigin,
    /// Subresource integrity builder
    sri: SriBuilder,
    /// If exporting Rust functions should be imported
    import_bindings: bool,
    /// Name of the global variable holding the imported WASM bindings
    import_bindings_name: Option<String>,
    /// The name of the initializer module
    initializer: Option<PathBuf>,
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
        let bin = attrs.get("data-bin").map(|attr| attr.to_string());
        let target_name = attrs.get("data-target-name").map(|attr| attr.to_string());
        let keep_debug = attrs.contains_key("data-keep-debug");
        let typescript = attrs.contains_key("data-typescript");
        let no_demangle = attrs.contains_key("data-no-demangle");
        let app_type = attrs
            .get("data-type")
            .map(|attr| attr.parse())
            .transpose()?
            .unwrap_or(RustAppType::Main);
        let reference_types = attrs.contains_key("data-reference-types");
        let weak_refs = attrs.contains_key("data-weak-refs");
        let wasm_opt = attrs
            .get("data-wasm-opt")
            .map(|attr| attr.parse())
            .transpose()?
            .unwrap_or_else(|| {
                if cfg.release {
                    Default::default()
                } else {
                    WasmOptLevel::Off
                }
            });
        let wasm_opt_params = attrs
            .get("data-wasm-opt-params")
            .iter()
            .flat_map(|attr| attr.split_whitespace())
            .map(|val| val.to_string())
            .collect();
        let wasm_bindgen_target = attrs
            .get("data-bindgen-target")
            .map(|attr| attr.parse())
            .transpose()?
            .unwrap_or(match app_type {
                RustAppType::Main => WasmBindgenTarget::Web,
                RustAppType::Worker => WasmBindgenTarget::NoModules,
            });
        let cross_origin = attrs
            .get("data-cross-origin")
            .map(|attr| CrossOrigin::from_str(attr))
            .transpose()?
            .unwrap_or_default();
        let integrity = IntegrityType::from_attrs(&attrs, &cfg)?;

        let manifest = CargoMetadata::new(&manifest_href).await?;
        let id = Some(id);
        let name = bin.clone().unwrap_or_else(|| manifest.package.name.clone());

        let loader_shim = attrs.contains_key("data-loader-shim");
        if loader_shim {
            ensure!(
                app_type == RustAppType::Worker,
                "Loader shim has no effect when data-type is \"main\"!"
            );
        }

        // cargo profile

        let data_cargo_profile = match cfg.release {
            true => attrs.get("data-cargo-profile-release"),
            false => attrs.get("data-cargo-profile-dev"),
        }
        .or_else(|| attrs.get("data-cargo-profile"));

        let cargo_profile = match data_cargo_profile {
            Some(cargo_profile) => {
                let cargo_profile = &cargo_profile.value;
                if let Some(config_cargo_profile) = &cfg.cargo_profile {
                    log::warn!("Cargo profile from configuration ({config_cargo_profile}) will be overridden with HTML file's more specific setting ({})", cargo_profile);
                }
                Some(cargo_profile.clone())
            }
            None => cfg.cargo_profile.as_ref().cloned(),
        };

        // cargo features

        let data_features = attrs
            .get("data-cargo-features")
            .map(|attr| attr.to_string());
        let data_all_features = attrs.contains_key("data-cargo-all-features");
        let data_no_default_features = attrs.contains_key("data-cargo-no-default-features");

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

        // skip

        let skip_build = attrs.contains_key("data-trunk-skip");

        // bindings

        let import_bindings = !attrs.contains_key("data-wasm-no-import");
        let import_bindings_name = attrs
            .get("data-wasm-import-name")
            .map(|attr| &attr.value)
            .cloned();

        // progress function

        let initializer = attrs
            .get("data-initializer")
            .map(|path| PathBuf::from_str(path))
            .transpose()?
            .map(|path| {
                if !path.is_absolute() {
                    html_dir.join(path)
                } else {
                    path
                }
            });

        let target_path = data_target_path(&attrs)?;

        // done

        Ok(Self {
            id,
            cfg,
            skip_build,
            cargo_profile,
            cargo_features,
            manifest,
            ignore_chan,
            bin,
            target_name,
            keep_debug,
            typescript,
            no_demangle,
            reference_types,
            weak_refs,
            wasm_opt,
            wasm_opt_params,
            wasm_bindgen_target,
            app_type,
            name,
            loader_shim,
            cross_origin,
            sri: SriBuilder::new(integrity),
            import_bindings,
            import_bindings_name,
            initializer,
            target_path,
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
        let integrity = IntegrityType::default_unless(cfg.no_sri);

        Ok(Some(Self {
            id: None,
            skip_build: false,
            cargo_features: cfg.cargo_features.clone(),
            cargo_profile: cfg.cargo_profile.clone(),
            cfg,
            manifest,
            ignore_chan,
            bin: None,
            target_name: None,
            keep_debug: false,
            typescript: false,
            no_demangle: false,
            reference_types: false,
            weak_refs: false,
            wasm_opt: WasmOptLevel::Off,
            wasm_opt_params: Default::default(),
            app_type: RustAppType::Main,
            wasm_bindgen_target: WasmBindgenTarget::Web,
            name,
            loader_shim: false,
            cross_origin: Default::default(),
            sri: SriBuilder::new(integrity),
            import_bindings: true,
            import_bindings_name: None,
            initializer: None,
            target_path: None,
        }))
    }

    /// Spawn a new pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    pub fn spawn(self) -> JoinHandle<Result<TrunkAssetPipelineOutput>> {
        tokio::spawn(self.build())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn build(mut self) -> Result<TrunkAssetPipelineOutput> {
        if self.skip_build {
            return Ok(TrunkAssetPipelineOutput::None);
        }

        // run the cargo build
        let wasm = self.cargo_build().await.context("running cargo build")?;

        // run wasm-bindgen
        let mut output = self
            .wasm_bindgen_build(&wasm)
            .await
            .context("running wasm-bindgen")?;

        // (optionally) run wasm-opt
        self.wasm_opt_build(&output.wasm_output)
            .await
            .context("running wasm-opt")?;

        // evaluate wasm integrity after all processing
        self.final_digest(&mut output)
            .await
            .with_context(|| format!("finalizing digest for '{}'", output.wasm_output))?;

        // now the build is complete
        tracing::debug!("rust build complete");
        Ok(TrunkAssetPipelineOutput::RustApp(output))
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn cargo_build(&mut self) -> Result<PathBuf> {
        tracing::debug!("building {}", &self.manifest.package.name);

        // Spawn the cargo build process.
        let mut args = vec![
            "build",
            "--target=wasm32-unknown-unknown",
            "--manifest-path",
            &self.manifest.manifest_path,
        ];
        if let Some(profile) = &self.cargo_profile {
            args.push("--profile");
            args.push(profile);
        } else if self.cfg.release {
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
        if let Some(example) = &self.cfg.cargo_example {
            args.push("--example");
            args.push(example);
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

        let build_res = common::run_command("cargo", "cargo", &args, &self.cfg.working_directory)
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
        tracing::debug!("fetching cargo artifacts");
        args.push("--message-format=json");
        let artifacts_out = Command::new("cargo")
            .current_dir(&self.cfg.core.working_directory)
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
        let mut artifacts: Vec<Artifact> = cargo_metadata::Message::parse_stream(reader)
            .filter_map(|msg| msg.ok())
            .filter_map(|msg| {
                tracing::trace!("Cargo message: {msg:?}");
                match msg {
                    cargo_metadata::Message::CompilerArtifact(art)
                        if self.is_relevant_artifact(&art) =>
                    {
                        Some(Ok(art))
                    }
                    cargo_metadata::Message::BuildFinished(finished) if !finished.success => {
                        Some(Err(anyhow!("error while fetching cargo artifact info")))
                    }
                    _ => None,
                }
            })
            .collect::<Result<_>>()?;
        // If there is already a `link data-trunk rel=rust` in index.html
        // then the --bin flag was passed to the cargo command
        // and it has built just a single binary
        if artifacts.len() > 1 {
            bail!(
                r#"found more than one target artifact: {names:?}:
 * consider adding `<link data-trunk rel="rust" data-bin={{bin}} />` to the index.html to build only the specified binary
 * or adding `<link data-trunk rel="rust" data-target-name={{artifact}} />` to select the specific artifact by name"#,
                names = artifacts.iter().map(|a| &a.target.name).collect::<Vec<_>>()
            )
        }
        let Some(artifact) = artifacts.pop() else {
            bail!("cargo artifacts not found for target crate")
        };

        // From the output artifact, find the path to the WASM file
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .context("could not find WASM output after cargo build")?;

        Ok(wasm.into_std_path_buf())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn wasm_bindgen_build(&mut self, wasm_path: &Path) -> Result<RustAppOutput> {
        let version = find_wasm_bindgen_version(&self.cfg.tools, &self.manifest);
        let ToolInformation {
            path: wasm_bindgen,
            version,
        } = tools::get_info(
            Application::WasmBindgen,
            version.as_deref(),
            self.cfg.offline,
            &self.cfg.client_options(),
        )
        .await?;
        let wasm_bindgen_features = WasmBindgenFeatures::from_version(&version)?;

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
        let arg_out_name = format!("--out-name={}", &self.name);
        let target_wasm = wasm_path.to_string_lossy().to_string();
        let target_type = format!("--target={}", self.wasm_bindgen_target);

        let mut args: Vec<&str> = vec![&target_type, &arg_out_path, &arg_out_name, &target_wasm];
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

        // the final base
        let target_path =
            target_path(&self.cfg.staging_dist, self.target_path.as_deref(), None).await?;

        // Invoke wasm-bindgen.
        tracing::debug!("calling wasm-bindgen for {}", self.name);
        common::run_command(
            wasm_bindgen_name,
            &wasm_bindgen,
            &args,
            &self.cfg.working_directory,
        )
        .await
        .map_err(|err| check_target_not_found_err(err, wasm_bindgen_name))?;

        // Copy the generated WASM & JS loader to the dist dir.
        tracing::debug!("copying generated wasm-bindgen artifacts");
        let hashed_name = self.hashed_wasm_base(wasm_path).await?;
        let hashed_wasm_name =
            apply_data_target_path(format!("{hashed_name}_bg.wasm"), &self.target_path);

        let js_name = format!("{}.js", self.name);
        let hashed_js_name =
            apply_data_target_path(format!("{}.js", hashed_name), &self.target_path);
        let ts_name = format!("{}.d.ts", self.name);
        let hashed_ts_name =
            apply_data_target_path(format!("{}.d.ts", hashed_name), &self.target_path);

        let js_loader_path = bindgen_out.join(&js_name);
        let js_loader_path_dist = self.cfg.staging_dist.join(&hashed_js_name);
        let wasm_name = format!("{}_bg.wasm", self.name);
        let wasm_path = bindgen_out.join(&wasm_name);
        let wasm_path_dist = self.cfg.staging_dist.join(&hashed_wasm_name);

        let hashed_loader_name = self.loader_shim.then(|| {
            apply_data_target_path(format!("{}_loader.js", hashed_name), &self.target_path)
        });
        let loader_shim_path = hashed_loader_name
            .as_ref()
            .map(|m| self.cfg.staging_dist.join(m));

        tracing::debug!(
            "copying {js_loader_path} to {}",
            js_loader_path_dist.display()
        );
        self.copy_or_minify_js(
            js_loader_path,
            &js_loader_path_dist,
            match self.wasm_bindgen_target {
                WasmBindgenTarget::NoModules => TopLevelMode::Global,
                _ => TopLevelMode::Module,
            },
        )
        .await
        .context("error minifying or copying JS loader file to stage dir")?;

        tracing::debug!("copying {wasm_path} to {}", wasm_path_dist.display());

        fs::copy(wasm_path, &wasm_path_dist)
            .await
            .context("error copying wasm file to stage dir")?;

        if self.typescript {
            let ts_path = bindgen_out.join(&ts_name);
            let ts_path_dist = self.cfg.staging_dist.join(&hashed_ts_name);

            tracing::debug!("copying {ts_path} to {}", ts_path_dist.display());
            fs::copy(ts_path, ts_path_dist)
                .await
                .context("error copying TS files to stage dir")?;
        }

        if let Some(ref m) = loader_shim_path {
            tracing::debug!("creating {}", m.display());
            let mut loader_f = fs::File::create(m)
                .await
                .context("error creating loader shim script")?;

            let shim = match self.wasm_bindgen_target {
                WasmBindgenTarget::Web => {
                    format!("import init from './{hashed_js_name}';await init();")
                }
                WasmBindgenTarget::NoModules => format!(
                    r#"importScripts("./{hashed_js_name}");wasm_bindgen("./{hashed_wasm_name}");"#,
                ),
                _ => bail!(
                    "Loader shim can only be created for data-bindgen-target \"web\" or \
                     \"no-modules\"!"
                ),
            };
            loader_f
                .write_all(shim.as_bytes())
                .await
                .context("error writing loader shim script")?;
            loader_f
                .flush()
                .await
                .context("error writing loader shim script")?;
        }

        // Check for any snippets, and copy them over.
        let snippets_dir_src = bindgen_out.join(SNIPPETS_DIR);
        let snippets = if path_exists(&snippets_dir_src).await? {
            let snippets_dir_dest = target_path.join(SNIPPETS_DIR);
            tracing::debug!(
                "recursively copying from '{snippets_dir_src}' to '{}'",
                snippets_dir_dest.display()
            );
            copy_dir_recursive(snippets_dir_src, snippets_dir_dest)
                .await
                .context("error copying snippets dir to stage dir")?
        } else {
            HashSet::new()
        };

        self.sri
            .record_file(
                SriType::ModulePreload,
                &hashed_js_name,
                SriOptions::default(),
                &js_loader_path_dist,
            )
            .await?;

        for snippet in snippets {
            if let Ok(name) = snippet.strip_prefix(&self.cfg.staging_dist) {
                self.sri
                    .record_file(
                        SriType::ModulePreload,
                        path_to_href(name),
                        SriOptions::default(),
                        &snippet,
                    )
                    .await?;
            }
        }

        // wasm size

        let wasm_size = fs::metadata(&wasm_path_dist).await?.len();

        // initializer

        let initializer = match &self.initializer {
            Some(initializer) => {
                let hashed_name = self.hashed_name(initializer).await?;
                let source = common::strip_prefix(initializer);
                let target = self.cfg.staging_dist.join(&hashed_name);

                self.copy_or_minify_js(source, &target, TopLevelMode::Module)
                    .await?;

                self.sri
                    .record_file(
                        SriType::ModulePreload,
                        &hashed_name,
                        SriOptions::default(),
                        &target,
                    )
                    .await?;

                Some(hashed_name)
            }
            None => None,
        };

        // return output

        Ok(RustAppOutput {
            id: self.id,
            cfg: self.cfg.clone(),
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
            wasm_size,
            r#type: self.app_type,
            cross_origin: self.cross_origin,
            integrities: self.sri.clone(),
            import_bindings: self.import_bindings,
            import_bindings_name: self.import_bindings_name.clone(),
            initializer,
            wasm_bindgen_features,
        })
    }

    /// create a cache busting hashed name based on a path, if enabled
    async fn hashed_name(&self, path: impl AsRef<Path>) -> Result<String> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .ok_or_else(|| anyhow!("Must be a file: {}", path.display()))?
            .to_string_lossy()
            .to_string();

        Ok(self
            .hashed(path)
            .await?
            .map(|hashed| format!("{hashed}-{name}"))
            .unwrap_or_else(|| name.clone()))
    }

    /// create a cache busting string, if enabled
    async fn hashed(&self, path: &Path) -> Result<Option<String>> {
        // generate a hashed name, just for cache busting
        Ok(match self.cfg.filehash {
            false => None,
            true => {
                tracing::debug!("processing hash for {}", path.display());

                let hash = {
                    let path = path.to_owned();
                    tokio::task::spawn_blocking(move || {
                        let mut file = std::fs::File::open(&path)?;
                        let mut hasher = SeaHasher::new();
                        std::io::copy(&mut file, &mut hasher).with_context(|| {
                            format!("error reading '{}' for hash generation", path.display())
                        })?;
                        Ok::<_, anyhow::Error>(hasher.finish())
                    })
                    .await??
                };

                Some(format!("{hash:x}"))
            }
        })
    }

    /// create a cache busting hashed name for the wasm file, if enabled.
    async fn hashed_wasm_base(&self, wasm: &Path) -> Result<String> {
        // Skip the hashed file name for workers as their file name must be named at runtime.
        // Therefore, workers use the Cargo binary name for file naming.
        if self.app_type == RustAppType::Worker {
            return Ok(self.name.clone());
        }

        Ok(self
            .hashed(wasm)
            .await?
            .map(|hashed| format!("{}-{hashed}", self.name))
            .unwrap_or_else(|| self.name.clone()))
    }

    fn is_relevant_artifact(&self, art: &Artifact) -> bool {
        // package id must match
        if art.package_id != self.manifest.package.id {
            return false;
        }

        // must be cdylib, bin, or example
        if !(art.target.kind.contains(&TargetKind::Bin)
            || art.target.kind.contains(&TargetKind::CDyLib)
            || art.target.kind.contains(&TargetKind::Example))
        {
            return false;
        }

        // Are we building an example?
        if let Some(example) = &self.cfg.cargo_example {
            // it must match
            if example != &art.target.name {
                return false;
            }
        }

        // if we have the --bin argument
        if let Some(bin) = &self.bin {
            // it must match
            if bin != &art.target.name {
                return false;
            }
        }

        // if we have a target name
        if let Some(target_name) = &self.target_name {
            if target_name != &art.target.name {
                return false;
            }
        }

        true
    }

    async fn copy_or_minify_js(
        &self,
        origin_path: impl AsRef<Path>,
        destination_path: &Path,
        mode: TopLevelMode,
    ) -> Result<()> {
        let bytes = fs::read(origin_path)
            .await
            .context("error reading JS loader file")?;

        let write_bytes = match self.cfg.should_minify() {
            true => minify_js(bytes, mode),
            false => bytes,
        };

        fs::write(destination_path, write_bytes)
            .await
            .context("error writing JS loader file to stage dir")?;

        Ok(())
    }

    /// Run `wasm-opt` on the `wasm_path` file, in-place.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn wasm_opt_build(&self, wasm_name: &str) -> Result<()> {
        // If not in release mode, we skip calling wasm-opt.
        if !self.cfg.release {
            return Ok(());
        }

        // If opt level is off, we skip calling wasm-opt as it wouldn't have any effect.
        if self.wasm_opt == WasmOptLevel::Off {
            log::debug!("wasm-opt is turned off");
            return Ok(());
        }

        let version = self.cfg.tools.wasm_opt.as_deref();
        let wasm_opt = tools::get(
            Application::WasmOpt,
            version,
            self.cfg.offline,
            &self.cfg.client_options(),
        )
        .await?;

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
        let output = output.join(format!("{}_bg.wasm", self.name));
        let arg_output = format!("--output={output}");
        let arg_opt_level = format!("-O{}", self.wasm_opt.as_ref());
        let arg_opt_params = self.wasm_opt_params.as_slice();
        let target_wasm = self
            .cfg
            .staging_dist
            .join(wasm_name)
            .to_string_lossy()
            .to_string();
        let mut args: Vec<&str> = vec![&arg_output, &arg_opt_level, &target_wasm];

        if self.reference_types {
            args.push("--enable-reference-types");
        }

        args.extend(arg_opt_params.iter().map(|s| s.as_str()));

        // Invoke wasm-opt.
        tracing::debug!("calling wasm-opt");
        common::run_command(wasm_opt_name, &wasm_opt, &args, &self.cfg.working_directory)
            .await
            .map_err(|err| check_target_not_found_err(err, wasm_opt_name))?;

        // Copy the generated WASM file to the dist dir.
        tracing::debug!("copying generated wasm-opt artifact from '{output}' to '{target_wasm}'");
        fs::copy(output, target_wasm)
            .await
            .context("error copying (optimized) wasm file to dist dir")?;

        Ok(())
    }

    /// Build the final WASM digest
    #[tracing::instrument(level = "trace", skip(self, output))]
    async fn final_digest(&self, output: &mut RustAppOutput) -> Result<()> {
        let final_wasm = self.cfg.staging_dist.join(&output.wasm_output);
        output
            .integrities
            .record_file(
                SriType::Preload,
                &output.wasm_output,
                SriOptions::default()
                    .r#as("fetch")
                    .r#type("application/wasm"),
                final_wasm,
            )
            .await?;

        Ok(())
    }
}
