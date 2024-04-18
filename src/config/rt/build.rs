use super::super::{DIST_DIR, STAGE_DIR};
use crate::config::{
    models::{BaseUrl, Minify},
    ConfigOptsBuild, ConfigOptsCore, ConfigOptsHook, ConfigOptsTools, RtcCore,
};
use anyhow::{ensure, Context};
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Arc;

/// Config options for the cargo build command
#[derive(Clone, Debug)]
pub enum Features {
    /// Use cargo's `--all-features` flag during compilation.
    All,
    /// An explicit list of features to use; might be empty; might include no-default-features.
    Custom {
        /// Space or comma separated list of cargo features to activate.
        features: Option<String>,
        /// Use cargo's `--no-default-features` flag during compilation.
        no_default_features: bool,
    },
}

/// Runtime config for the build system.
#[derive(Clone, Debug)]
pub struct RtcBuild {
    pub core: Arc<RtcCore>,
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// The parent directory of the target index HTML file.
    pub target_parent: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// Build without network access
    pub offline: bool,
    /// Require Cargo.lock and cache are up to date
    pub frozen: bool,
    /// Require Cargo.lock is up to date
    pub locked: bool,
    /// The public URL from which assets are to be served.
    pub public_url: BaseUrl,
    /// If `true`, then files being processed should be hashed and the hash should be
    /// appended to the file's name.
    pub filehash: bool,
    /// The directory where final build artifacts are placed after a successful build.
    pub final_dist: PathBuf,
    /// The directory used to stage build artifacts during an active build.
    pub staging_dist: PathBuf,
    /// The configuration of the features passed to cargo.
    pub cargo_features: Features,
    /// Configuration for automatic application download.
    pub tools: ConfigOptsTools,
    /// Build process hooks.
    pub hooks: Vec<ConfigOptsHook>,
    /// A bool indicating if the output HTML should have the WebSocket autoloader injected.
    ///
    /// This value is configured via the server config only. If the server is not being used, then
    /// the autoloader will not be injected.
    pub inject_autoloader: bool,
    /// A bool indication if the output HTML should have module preloads and scripts injected.
    pub inject_scripts: bool,
    /// Optional pattern for the app loader script.
    pub pattern_script: Option<String>,
    /// Optional pattern for the app preload element.
    pub pattern_preload: Option<String>,
    /// Optional replacement parameters corresponding to the patterns provided in
    /// `pattern_script` and `pattern_preload`.
    pub pattern_params: Option<HashMap<String, String>>,
    /// Optional root certificate chain for use when downloading dependencies.
    pub root_certificate: Option<PathBuf>,
    /// Sets if reqwest is allowed to ignore certificate validation errors (defaults to false).
    ///
    /// **WARNING**: Setting this to true can make you vulnerable to man-in-the-middle attacks. Sometimes this is necessary when working behind corporate proxies.
    pub accept_invalid_certs: Option<bool>,
    /// Control minification
    pub minify: Minify,
    /// Allow disabling SRI
    pub no_sri: bool,
    /// Ignore error's due to self-closed script tags, instead will issue a warning.
    pub allow_self_closing_script: bool,
}

impl RtcBuild {
    /// Construct a new instance.
    pub(crate) fn new(
        core: ConfigOptsCore,
        opts: ConfigOptsBuild,
        tools: ConfigOptsTools,
        hooks: Vec<ConfigOptsHook>,
        inject_autoloader: bool,
    ) -> anyhow::Result<Self> {
        let core = Arc::new(RtcCore::new(core));

        // Get the canonical path to the target HTML file.
        let mut pre_target = opts.target.clone().unwrap_or_else(|| "index.html".into());
        if !pre_target.is_absolute() {
            pre_target = core.working_directory.join(pre_target);
        }
        let target = pre_target.canonicalize().with_context(|| {
            format!(
                "error getting canonical path to source HTML file {:?}",
                &pre_target
            )
        })?;

        // Get the target HTML's parent dir, falling back to OS specific root, as that is the only
        // time when no parent could be determined.
        let target_parent = target
            .parent()
            .map(|path| path.to_owned())
            .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));

        // Ensure the final dist dir exists and that we have a canonical path to the dir. Normally
        // we would want to avoid such an action at this layer, however to ensure that other layers
        // have a reliable FS path to work with, we make an exception here.
        let final_dist = opts.dist.unwrap_or_else(|| target_parent.join(DIST_DIR));
        if !final_dist.exists() {
            std::fs::create_dir(&final_dist)
                .or_else(|err| {
                    if err.kind() == ErrorKind::AlreadyExists {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })
                .with_context(|| {
                    format!("error creating final dist directory {:?}", &final_dist)
                })?;
        }
        let final_dist = final_dist
            .canonicalize()
            .context("error taking canonical path to dist dir")?;
        let staging_dist = final_dist.join(STAGE_DIR);

        // Highlander-rule: There can be only one (prohibits contradicting arguments):
        ensure!(
            !(opts.all_features && (opts.no_default_features || opts.features.is_some())),
            "Cannot combine --all-features with --no-default-features and/or --features"
        );

        let cargo_features = if opts.all_features {
            Features::All
        } else {
            Features::Custom {
                features: opts.features,
                no_default_features: opts.no_default_features,
            }
        };

        let mut public_url = opts.public_url.unwrap_or_default();
        if !opts.public_url_no_trailing_slash_fix {
            public_url = public_url.fix_trailing_slash();
        }

        let minify = match (opts.minify_cli, opts.minify_toml) {
            // the CLI will override with "always"
            (true, _) => Minify::Always,
            // otherwise, we take the configuration value, or the default
            (false, minify) => minify.unwrap_or_default(),
        };

        Ok(Self {
            core,
            target,
            target_parent,
            release: opts.release,
            public_url,
            filehash: opts.filehash.unwrap_or(true),
            staging_dist,
            final_dist,
            cargo_features,
            tools,
            hooks,
            inject_autoloader,
            inject_scripts: opts.inject_scripts.unwrap_or(true),
            pattern_script: opts.pattern_script,
            pattern_preload: opts.pattern_preload,
            pattern_params: opts.pattern_params,
            offline: opts.offline,
            frozen: opts.frozen,
            locked: opts.locked,
            root_certificate: opts.root_certificate.map(PathBuf::from),
            accept_invalid_certs: opts.accept_invalid_certs,
            minify,
            no_sri: opts.no_sri,
            allow_self_closing_script: opts.allow_self_closing_script,
        })
    }

    /// Construct a new instance for testing.
    #[cfg(test)]
    pub async fn new_test(tmpdir: &std::path::Path) -> anyhow::Result<Self> {
        let target = tmpdir.join("index.html");
        let target_parent = tmpdir.to_path_buf();
        let final_dist = tmpdir.join("dist");
        let staging_dist = final_dist.join(".stage");
        tokio::fs::create_dir_all(&staging_dist)
            .await
            .context("error creating dist & staging dir for test")?;
        Ok(Self {
            core: Arc::new(RtcCore::new_test()),
            target,
            target_parent,
            release: false,
            public_url: Default::default(),
            filehash: true,
            final_dist,
            staging_dist,
            cargo_features: Features::All,
            tools: ConfigOptsTools {
                sass: None,
                wasm_bindgen: None,
                wasm_opt: None,
                tailwindcss: None,
            },
            hooks: Vec::new(),
            inject_autoloader: true,
            inject_scripts: true,
            pattern_script: None,
            pattern_preload: None,
            pattern_params: None,
            offline: false,
            frozen: false,
            locked: false,
            root_certificate: None,
            accept_invalid_certs: None,
            minify: Minify::Never,
            no_sri: false,
            allow_self_closing_script: false,
        })
    }

    /// Evaluate the minify state with an asset's no_minify setting.
    pub fn minify_asset(&self, no_minify: bool) -> bool {
        !no_minify && self.should_minify()
    }

    /// Evaluate a global minify state, assets might override this.
    pub fn should_minify(&self) -> bool {
        match (self.minify, self.release) {
            (Minify::Never, _) => false,
            (Minify::OnRelease, release) => release,
            (Minify::Always, _) => true,
        }
    }
}
