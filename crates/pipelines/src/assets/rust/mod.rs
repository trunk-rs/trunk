//! Rust application pipeline.
use std::borrow::Cow;
use std::iter::Iterator;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;

use cargo_lock::Lockfile;
use futures_util::stream::{self, BoxStream};
use futures_util::{FutureExt, StreamExt, TryStreamExt};
use nipper::Document;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
mod cargo;
mod config;
mod wasm_opt;
use async_trait::async_trait;
use cargo::RustAppType;
pub use config::RustAppConfig;
use trunk_util::AssetInput;
use wasm_opt::WasmOptLevel;

use super::{Asset, Output};
use crate::tools::Application;
use crate::util::{
    copy_dir_recursive, path_exists, trunk_id_selector, CargoMetadata, ErrorReason, Executable,
    Features, Result, ResultExt, ATTR_HREF, ATTR_REL, SNIPPETS_DIR,
};

static TYPE_RUST_APP: &str = "rust";

#[derive(Debug)]
struct Input {
    asset_input: Option<AssetInput>,

    /// The configuration of the features passed to cargo.
    cargo_features: Features,
    /// Is this module main or a worker.
    app_type: RustAppType,
    /// All metadata associated with the target Cargo project.
    manifest: CargoMetadata,

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
}

impl Input {
    async fn try_from<C>(cfg: &C, input: AssetInput) -> Result<Self>
    where
        C: RustAppConfig,
    {
        if input.attrs.get(ATTR_REL).map(|m| m.as_str()) != Some(TYPE_RUST_APP) {
            return Err(ErrorReason::AssetNotMatched { input }.into_error());
        }

        let attrs = &input.attrs;

        // Build the path to the target asset.
        let manifest_href = attrs
            .get(ATTR_HREF)
            .map(|attr| {
                let mut path = PathBuf::new();
                path.extend(attr.split('/'));
                if !path.is_absolute() {
                    path = input.manifest_dir.join(path);
                }
                if !path.ends_with("Cargo.toml") {
                    path = path.join("Cargo.toml");
                }
                path
            })
            .unwrap_or_else(|| input.manifest_dir.join("Cargo.toml"));
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
                if cfg.should_optimize() {
                    Default::default()
                } else {
                    WasmOptLevel::Off
                }
            });
        let manifest = CargoMetadata::new(&manifest_href).await?;
        let name = bin.clone().unwrap_or_else(|| manifest.package.name.clone());

        let data_features = attrs.get("data-cargo-features").map(|val| val.to_string());
        let data_all_features = attrs.get("data-cargo-all-features").is_some();
        let data_no_default_features = attrs.get("data-cargo-no-default-features").is_some();

        let loader_shim = attrs.get("data-loader-shim").is_some();
        if loader_shim && app_type == RustAppType::Worker {
            return Err(ErrorReason::RustUselessShim.into_error());
        }

        // Highlander-rule: There can be only one (prohibits contradicting arguments):
        if !(data_all_features && (data_no_default_features || data_features.is_some())) {
            return Err(ErrorReason::CargoFeatureConflict.into_error());
        }

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
            cfg.cargo_features().cloned().unwrap_or_default()
        };

        let input = Input {
            asset_input: Some(input),
            cargo_features,
            manifest,
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
        };

        Ok(input)
    }
}

/// A Rust application pipeline.
pub struct RustApp<C> {
    /// Runtime config.
    cfg: Arc<C>,
    /// An optional channel to be used to communicate paths to ignore back to the watcher.
    ignore_chan: Option<mpsc::Sender<PathBuf>>,

    default_manifest_dir: Option<PathBuf>,

    inputs: Vec<Input>,
}

impl<C> RustApp<C>
where
    C: RustAppConfig,
{
    pub fn new(cfg: Arc<C>) -> Self {
        Self {
            cfg,
            ignore_chan: None,
            default_manifest_dir: None,
            inputs: Vec::new(),
        }
    }

    // pub async fn new_default(cfg: Arc<C>, html_dir: Arc<PathBuf>) -> Result<Self> {

    //     Ok(Self {

    //     })
    // }

    /// Register a channel that would receive the target directory to ignore.
    pub fn ignore_chan(mut self, ignore_chan: mpsc::Sender<PathBuf>) -> Self {
        self.ignore_chan = Some(ignore_chan);
        self
    }

    /// Ensures a default app is produced if no rust app input is pushed for processing.
    pub fn ensure_main_app<P>(mut self, manifest_dir: P) -> Self
    where
        P: Into<PathBuf>,
    {
        self.default_manifest_dir = Some(manifest_dir.into());

        self
    }

    #[tracing::instrument(level = "trace", skip(cfg))]
    async fn run_with_input(
        cfg: Arc<C>,
        input: Input,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
    ) -> Result<RustAppOutput<C>> {
        let (wasm, hashed_name) = Self::cargo_build(cfg.as_ref(), &input, ignore_chan).await?;
        let output =
            Self::wasm_bindgen_build(cfg.clone(), &input, wasm.as_ref(), &hashed_name).await?;
        Self::wasm_opt_build(cfg.as_ref(), &input, &output.wasm_output).await?;
        Ok(output)
    }

    #[tracing::instrument(level = "trace", skip(cfg))]
    async fn cargo_build(
        cfg: &C,
        input: &Input,
        ignore_chan: Option<mpsc::Sender<PathBuf>>,
    ) -> Result<(PathBuf, String)> {
        tracing::info!("building {}", &input.manifest.package.name);

        // Spawn the cargo build process.
        let mut args = vec![
            "build",
            "--target=wasm32-unknown-unknown",
            "--manifest-path",
            &input.manifest.manifest_path,
        ];
        if cfg.should_optimize() {
            args.push("--release");
        }
        if let Some(bin) = &input.bin {
            args.push("--bin");
            args.push(bin);
        }

        match &input.cargo_features {
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

        let build_res = Executable::new(Path::new("cargo"))
            .with_name("cargo")
            .run_with_args(&args)
            .await
            .reason(ErrorReason::CargoBuildFailed);

        // Send cargo's target dir over to the watcher to be ignored. We must do this before
        // checking for errors, otherwise the dir will never be ignored. If we attempt to do
        // this pre-build, the canonicalization will fail and will not be ignored.
        if let Some(chan) = ignore_chan {
            let _ = chan.try_send(
                input
                    .manifest
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
            .reason(ErrorReason::CargoArtifactReadFailed)?
            .wait_with_output()
            .await
            .reason(ErrorReason::CargoArtifactReadFailed)?;
        if !artifacts_out.status.success() {
            eprintln!("{}", String::from_utf8_lossy(&artifacts_out.stderr));
            return Err(ErrorReason::CargoArtifactReadFailed.into_error());
        }

        // Stream over cargo messages to find the artifacts we are interested in.
        let reader = std::io::BufReader::new(artifacts_out.stdout.as_slice());
        let mut bin_artifacts: Vec<cargo_metadata::Artifact> =
            cargo_metadata::Message::parse_stream(reader)
                .filter_map(|msg| msg.ok())
                .filter_map(|msg| match msg {
                    cargo_metadata::Message::CompilerArtifact(art)
                        if art.package_id == input.manifest.package.id
                            && art.target.kind.iter().any(|k| k == "bin") =>
                    {
                        Some(Ok(art))
                    }
                    cargo_metadata::Message::BuildFinished(finished) if !finished.success => {
                        Some(Err(ErrorReason::CargoArtifactReadFailed.into_error()))
                    }
                    _ => None,
                })
                .collect::<Result<_>>()?;
        // If there is already a `link data-trunk rel=rust` in index.html
        // then the --bin flag was passed to the cargo command
        // and it has built just a single binary
        if bin_artifacts.len() > 1 {
            return Err(ErrorReason::CargoManyArtifactFound {
                bin_names: bin_artifacts
                    .iter()
                    .map(|a| a.target.name.clone())
                    .collect::<Vec<_>>(),
            }
            .into_error());
        }
        let Some(artifact) = bin_artifacts.pop() else {
            return Err(ErrorReason::CargoArtifactNotFound.into_error())?;
        };

        // Get a handle to the WASM output file.
        let wasm = artifact
            .filenames
            .into_iter()
            .find(|path| path.extension().map(|ext| ext == "wasm").unwrap_or(false))
            .reason(ErrorReason::CargoWasmArtifactNotFound)?;

        // Hash the built wasm app, then use that as the out-name param.
        tracing::info!("processing WASM for {}", input.name);
        let wasm_bytes = fs::read(&wasm)
            .await
            .with_reason(|| ErrorReason::FsReadFailed {
                path: wasm.clone().into_std_path_buf(),
            })?;
        let hashed_name = cfg
            .should_hash()
            .then(|| format!("{}-{:x}", input.name, seahash::hash(&wasm_bytes)))
            .unwrap_or_else(|| input.name.clone());

        Ok((wasm.into_std_path_buf(), hashed_name))
    }

    #[tracing::instrument(level = "trace", skip(cfg, wasm, hashed_name))]
    async fn wasm_bindgen_build(
        cfg: Arc<C>,
        input: &Input,
        wasm: &Path,
        hashed_name: &str,
    ) -> Result<RustAppOutput<C>> {
        // Skip the hashed file name for workers as their file name must be named at runtime.
        // Therefore, workers use the Cargo binary name for file naming.
        let hashed_name = match input.app_type {
            RustAppType::Main => hashed_name,
            RustAppType::Worker => &input.name,
        };

        let version = find_wasm_bindgen_version(cfg.as_ref(), &input.manifest);
        let app = Application::WASM_BINDGEN;
        let wasm_bindgen = app.get(version.as_deref()).await?;

        // Ensure our output dir is in place.
        let wasm_bindgen_name = app.name();
        let mode_segment = if cfg.should_optimize() {
            "release"
        } else {
            "debug"
        };
        let bindgen_out = input
            .manifest
            .metadata
            .target_directory
            .join(wasm_bindgen_name)
            .join(mode_segment);
        fs::create_dir_all(bindgen_out.as_path())
            .await
            .with_reason(|| ErrorReason::FsCreateDirFailed {
                path: bindgen_out.clone().into_std_path_buf(),
            })?;

        // Build up args for calling wasm-bindgen.
        let arg_out_path = format!("--out-dir={}", bindgen_out);
        let arg_out_name = format!("--out-name={}", &hashed_name);
        let target_wasm = wasm.to_string_lossy().to_string();
        let target_type = match input.app_type {
            RustAppType::Main => "--target=web",
            RustAppType::Worker => "--target=no-modules",
        };

        let mut args = vec![target_type, &arg_out_path, &arg_out_name, &target_wasm];
        if input.keep_debug {
            args.push("--keep-debug");
        }
        if input.no_demangle {
            args.push("--no-demangle");
        }
        if input.reference_types {
            args.push("--reference-types");
        }
        if input.weak_refs {
            args.push("--weak-refs");
        }

        if !input.typescript {
            args.push("--no-typescript");
        }

        // Invoke wasm-bindgen.
        tracing::info!("calling wasm-bindgen for {}", input.name);
        wasm_bindgen.run_with_args(&args).await?;

        // Copy the generated WASM & JS loader to the dist dir.
        tracing::info!("copying generated wasm-bindgen artifacts");
        let hashed_js_name = format!("{}.js", &hashed_name);
        let hashed_wasm_name = format!("{}_bg.wasm", &hashed_name);
        let hashed_ts_name = format!("{}.d.ts", &hashed_name);
        let js_loader_path = bindgen_out.join(&hashed_js_name);
        let js_loader_path_dist = cfg.output_dir().join(&hashed_js_name);
        let wasm_path = bindgen_out.join(&hashed_wasm_name);
        let wasm_path_dist = cfg.output_dir().join(&hashed_wasm_name);
        let hashed_loader_name = input
            .loader_shim
            .then(|| format!("{}_loader.js", &hashed_name));
        let loader_shim_path = hashed_loader_name
            .as_ref()
            .map(|m| cfg.output_dir().join(m));

        fs::copy(&js_loader_path, &js_loader_path_dist)
            .await
            .with_reason(|| ErrorReason::FsCopyFailed {
                from_path: js_loader_path.clone().into_std_path_buf(),
                to_path: js_loader_path_dist,
            })?;
        fs::copy(&wasm_path, &wasm_path_dist)
            .await
            .with_reason(|| ErrorReason::FsCopyFailed {
                from_path: wasm_path.clone().into_std_path_buf(),
                to_path: wasm_path_dist.to_owned(),
            })?;

        if input.typescript {
            let ts_path = bindgen_out.join(&hashed_ts_name);
            let ts_path_dist = cfg.output_dir().join(&hashed_ts_name);

            fs::copy(&ts_path, &ts_path_dist)
                .await
                .with_reason(|| ErrorReason::FsCopyFailed {
                    from_path: ts_path.into_std_path_buf(),
                    to_path: ts_path_dist,
                })?;
        }

        if let Some(ref m) = loader_shim_path {
            let mut loader_f = fs::File::create(m)
                .await
                .with_reason(|| ErrorReason::FsWriteFailed { path: m.clone() })?;

            loader_f
                .write_all(
                    format!(
                        r#"importScripts("./{}");wasm_bindgen("./{}");"#,
                        hashed_js_name, hashed_wasm_name
                    )
                    .as_bytes(),
                )
                .await
                .with_reason(|| ErrorReason::FsWriteFailed { path: m.to_owned() })?;
            loader_f
                .flush()
                .await
                .with_reason(|| ErrorReason::FsWriteFailed { path: m.to_owned() })?;
        }

        let ts_output = if input.typescript {
            Some(hashed_ts_name)
        } else {
            None
        };

        // Check for any snippets, and copy them over.
        let snippets_dir = bindgen_out.join(SNIPPETS_DIR);
        if path_exists(&snippets_dir).await? {
            copy_dir_recursive(
                bindgen_out.join(SNIPPETS_DIR),
                cfg.output_dir().join(SNIPPETS_DIR),
            )
            .await
            .with_reason(|| ErrorReason::FsCopyFailed {
                from_path: bindgen_out.join(SNIPPETS_DIR).into_std_path_buf(),
                to_path: cfg.output_dir().join(SNIPPETS_DIR),
            })?;
        }

        Ok(RustAppOutput {
            id: input.asset_input.as_ref().map(|m| m.id),
            cfg,
            js_output: hashed_js_name,
            wasm_output: hashed_wasm_name,
            ts_output,
            loader_shim_output: hashed_loader_name,
            type_: input.app_type,
        })
    }

    #[tracing::instrument(level = "trace", skip(cfg, hashed_name))]
    async fn wasm_opt_build(cfg: &C, input: &Input, hashed_name: &str) -> Result<()> {
        // If not in release mode, we skip calling wasm-opt.
        if !cfg.should_optimize() {
            return Ok(());
        }

        // If opt level is off, we skip calling wasm-opt as it wouldn't have any effect.
        if input.wasm_opt == WasmOptLevel::Off {
            return Ok(());
        }

        let version = cfg.wasm_opt_version();
        let app = Application::WASM_OPT;
        let wasm_opt = app.get(version).await?;

        // Ensure our output dir is in place.
        let wasm_opt_name = app.name();
        let mode_segment = if cfg.should_optimize() {
            "release"
        } else {
            "debug"
        };
        let output = input
            .manifest
            .metadata
            .target_directory
            .join(wasm_opt_name)
            .join(mode_segment);
        fs::create_dir_all(&output)
            .await
            .with_reason(|| ErrorReason::FsCreateDirFailed {
                path: output.to_path_buf().into_std_path_buf(),
            })?;

        // Build up args for calling wasm-opt.
        let output = output.join(hashed_name);
        let arg_output = format!("--output={}", output);
        let arg_opt_level = format!("-O{}", input.wasm_opt.as_ref());
        let target_wasm = cfg
            .output_dir()
            .join(hashed_name)
            .to_string_lossy()
            .to_string();
        let mut args: Vec<&str> = vec![&arg_output, &arg_opt_level, &target_wasm];

        if input.reference_types {
            args.push("--enable-reference-types");
        }

        // Invoke wasm-opt.
        tracing::info!("calling wasm-opt");
        wasm_opt.run_with_args(&args).await?;

        // Copy the generated WASM file to the dist dir.
        tracing::info!("copying generated wasm-opt artifacts");
        fs::copy(&output, cfg.output_dir().join(hashed_name))
            .await
            .with_reason(|| ErrorReason::FsCopyFailed {
                from_path: output.clone().into_std_path_buf(),
                to_path: cfg.output_dir().join(hashed_name),
            })?;

        Ok(())
    }
}

#[async_trait]
impl<C> Asset for RustApp<C>
where
    C: RustAppConfig + Sync + Send + 'static,
{
    type Output = RustAppOutput<C>;
    type OutputStream = BoxStream<'static, Result<Self::Output>>;

    async fn try_push_input(&mut self, input: AssetInput) -> Result<()> {
        let input = Input::try_from(self.cfg.as_ref(), input).await?;

        self.inputs.push(input);

        Ok(())
    }

    async fn run_once(&self, input: AssetInput) -> Result<Self::Output> {
        let input = Input::try_from(self.cfg.as_ref(), input).await?;
        Self::run_with_input(self.cfg.clone(), input, self.ignore_chan.clone()).await
    }

    fn outputs(self) -> Self::OutputStream {
        let Self {
            cfg,
            mut inputs,
            ignore_chan,
            default_manifest_dir,
        } = self;

        {
            let cfg = cfg.clone();

            stream::once(async move {
                if inputs.is_empty() {
                    if let Some(m) = default_manifest_dir {
                        let path = m.join("Cargo.toml");
                        let manifest = match CargoMetadata::new(&path).await {
                            Ok(m) => m,
                            Err(e) => return Err(e),
                        };
                        let name = manifest.package.name.clone();

                        inputs.push(Input {
                            asset_input: None,
                            cargo_features: cfg.cargo_features().cloned().unwrap_or_default(),
                            manifest,
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
                        });
                    }
                }

                Ok(stream::iter(inputs).map(Ok))
            })
        }
        .try_flatten()
        .and_then(move |input| {
            let cfg = cfg.clone();
            let allow_concurrent_build = cfg.allow_concurrent_cargo_build();
            let ignore_chan = ignore_chan.clone();
            let f = async move { Self::run_with_input(cfg, input, ignore_chan).await };

            if allow_concurrent_build {
                tokio::spawn(f)
                    .map(|m| match m.reason(ErrorReason::TokioTaskFailed) {
                        Ok(Ok(m)) => Ok(m),
                        Ok(Err(e)) | Err(e) => Err(e),
                    })
                    .left_future()
            } else {
                f.right_future()
            }
        })
        .boxed()
    }
}

/// Find the appropriate version of `wasm-bindgen` to use. The version can be found in 3 different
/// location in order:
/// - Defined in the `Trunk.toml` as highest priority.
/// - Located in the `Cargo.lock` if it exists. This is mostly the case as we run `cargo build`
///   before even calling this function.
/// - Located in the `Cargo.toml` as direct dependency of the project.
fn find_wasm_bindgen_version<'a, C>(cfg: &'a C, manifest: &CargoMetadata) -> Option<Cow<'a, str>>
where
    C: RustAppConfig,
{
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

    cfg.wasm_bindgen_version()
        .map(Cow::from)
        .or_else(find_lock)
        .or_else(find_manifest)
}

/// The output of a cargo build pipeline.
pub struct RustAppOutput<C> {
    /// The runtime build config.
    pub cfg: Arc<C>,
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
}

#[async_trait(?Send)]
impl<C> Output for RustAppOutput<C>
where
    C: RustAppConfig + Send + Sync,
{
    async fn finalize(self, dom: &mut Document) -> Result<()> {
        if self.type_ == RustAppType::Worker {
            // Skip the script tag and preload links for workers, and remove the link tag only.
            // Workers are initialized and managed by the app itself at runtime.
            if let Some(id) = self.id {
                dom.select(&trunk_id_selector(id)).remove();
            }
            return Ok(());
        }

        let (base, js, wasm, head, body) = (
            self.cfg.public_url(),
            &self.js_output,
            &self.wasm_output,
            "html head",
            "html body",
        );
        let script = self.cfg.format_script(js, wasm).unwrap_or_else(
            || format!(
                r#"<script type="module">import init from '{base}{js}';init('{base}{wasm}');</script>"#,
                base = base,
                js = js,
                wasm = wasm,
            )
        );

        let preload = self.cfg.format_preload(js, wasm).unwrap_or_else(|| {
            format!(
                r#"
                    <link rel="preload" href="{base}{wasm}" as="fetch" type="application/wasm" crossorigin>
                    <link rel="modulepreload" href="{base}{js}">
                "#,
                base = base,
                js = js,
                wasm = wasm
            )
        });

        dom.select(head).append_html(preload);
        match self.id {
            Some(id) => dom.select(&trunk_id_selector(id)).replace_with_html(script),
            None => dom.select(body).append_html(script),
        }
        Ok(())
    }
}
