use super::{super::STAGE_DIR, RtcBuilder};
use crate::{
    config::{
        models::{Configuration, Hook, Tools},
        rt::{CoreOptions, RtcCore},
        types::{BaseUrl, Minify},
        Hooks,
    },
    tools::HttpClientOptions,
};
use anyhow::{ensure, Context};
use std::{collections::HashMap, ops::Deref, path::PathBuf};

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
    pub core: RtcCore,
    /// The index HTML file to drive the bundling process.
    pub target: PathBuf,
    /// The name of the output HTML file.
    pub html_output_filename: String,
    /// The parent directory of the target index HTML file.
    pub target_parent: PathBuf,
    /// Build in release mode.
    pub release: bool,
    /// Cargo profile to use instead of the default selection.
    pub cargo_profile: Option<String>,
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
    /// Optional example to be passed to cargo.
    pub cargo_example: Option<String>,
    /// Configuration for automatic application download.
    pub tools: Tools,
    /// Build process hooks.
    pub hooks: Vec<Hook>,
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
    pub pattern_params: HashMap<String, String>,
    /// Optional root certificate chain for use when downloading dependencies.
    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    pub root_certificate: Option<PathBuf>,
    /// Sets if reqwest is allowed to ignore certificate validation errors (defaults to false).
    ///
    /// **WARNING**: Setting this to true can make you vulnerable to man-in-the-middle attacks. Sometimes this is necessary when working behind corporate proxies.
    #[cfg(any(feature = "native-tls", feature = "rustls"))]
    pub accept_invalid_certs: bool,
    /// Control minification
    pub minify: Minify,
    /// Allow disabling SRI
    pub no_sri: bool,
    /// Ignore error's due to self-closed script tags, instead will issue a warning.
    pub allow_self_closing_script: bool,
    /// When set, create nonce attributes with the option as placeholder
    pub create_nonce: Option<String>,
}

impl Deref for RtcBuild {
    type Target = RtcCore;

    fn deref(&self) -> &Self::Target {
        &self.core
    }
}

#[derive(Clone, Debug)]
pub struct BuildOptions {
    pub core: CoreOptions,
    pub inject_autoloader: bool,
}

impl RtcBuild {
    /// Construct a new instance.
    pub(crate) fn new(config: Configuration, opts: BuildOptions) -> anyhow::Result<Self> {
        let BuildOptions {
            core: core_opts,
            inject_autoloader,
        } = opts;

        let Configuration {
            core: core_config,
            build,
            tools,
            hooks: Hooks(hooks),
            ..
        } = config;

        let core = RtcCore::new(core_config, core_opts)?;

        // Get the canonical path to the target HTML file.
        let mut pre_target = build.target.clone();
        if !pre_target.is_absolute() {
            pre_target = core.working_directory.join(pre_target);
        }
        let target = pre_target.canonicalize().with_context(|| {
            format!(
                "error getting the canonical path to the build target HTML file {:?}",
                &pre_target
            )
        })?;

        let html_output_filename = match build.html_output {
            Some(html_output) => html_output,
            None => target
                .file_name()
                .context("target path isn't a file path")?
                .to_string_lossy()
                .into_owned(),
        };

        // Get the target HTML's parent dir, falling back to OS specific root, as that is the only
        // time when no parent could be determined.
        let target_parent = target
            .parent()
            .map(|path| path.to_owned())
            .unwrap_or_else(|| PathBuf::from(std::path::MAIN_SEPARATOR.to_string()));

        // Ensure the final dist dir exists and that we have a canonical path to the dir. Normally
        // we would want to avoid such an action at this layer, however to ensure that other layers
        // have a reliable FS path to work with, we make an exception here.
        let final_dist = core.working_directory.join(&build.dist);

        std::fs::create_dir_all(&final_dist)
            .with_context(|| format!("error creating final dist directory {final_dist:?}"))?;

        let final_dist = final_dist
            .canonicalize()
            .context("error taking canonical path to dist dir")?;
        let staging_dist = final_dist.join(STAGE_DIR);

        // Highlander-rule: There can be only one (prohibits contradicting arguments):
        ensure!(
            !(build.all_features && (build.no_default_features || !build.features.is_empty())),
            "Cannot combine --all-features with --no-default-features and/or --features"
        );

        let cargo_features = if build.all_features {
            Features::All
        } else {
            Features::Custom {
                features: match build.features.is_empty() {
                    true => None,
                    false => Some(build.features.join(",")),
                },
                no_default_features: build.no_default_features,
            }
        };

        let mut public_url = build.public_url;
        if !build.public_url_no_trailing_slash_fix {
            public_url = public_url.fix_trailing_slash();
        }

        let create_nonce = build.create_nonce.then_some(build.nonce_placeholder);

        Ok(Self {
            core,
            target,
            html_output_filename,
            target_parent,
            release: build.release,
            cargo_profile: build.cargo_profile,
            public_url,
            filehash: build.filehash,
            staging_dist,
            final_dist,
            cargo_features,
            cargo_example: build.example,
            tools,
            hooks,
            inject_autoloader,
            inject_scripts: build.inject_scripts,
            pattern_script: build.pattern_script,
            pattern_preload: build.pattern_preload,
            pattern_params: build.pattern_params,
            offline: build.offline,
            frozen: build.frozen,
            locked: build.locked,
            #[cfg(any(feature = "native-tls", feature = "rustls"))]
            root_certificate: build.root_certificate.map(PathBuf::from),
            #[cfg(any(feature = "native-tls", feature = "rustls"))]
            accept_invalid_certs: build.accept_invalid_certs,
            minify: build.minify,
            no_sri: build.no_sri,
            allow_self_closing_script: build.allow_self_closing_script,
            create_nonce,
        })
    }

    /// Construct a new instance for testing.
    #[cfg(test)]
    pub async fn new_test(tmpdir: &std::path::Path) -> anyhow::Result<Self> {
        let target = tmpdir.join("index.html");
        let html_output_filename = String::from("index.html");
        let target_parent = tmpdir.to_path_buf();
        let final_dist = tmpdir.join("dist");
        let staging_dist = final_dist.join(".stage");
        tokio::fs::create_dir_all(&staging_dist)
            .await
            .context("error creating dist & staging dir for test")?;
        Ok(Self {
            core: RtcCore::new_test(tmpdir),
            target,
            html_output_filename,
            target_parent,
            release: false,
            cargo_profile: None,
            public_url: Default::default(),
            filehash: true,
            final_dist,
            staging_dist,
            cargo_features: Features::All,
            cargo_example: None,
            tools: Default::default(),
            hooks: Vec::new(),
            inject_autoloader: true,
            inject_scripts: true,
            pattern_script: None,
            pattern_preload: None,
            pattern_params: Default::default(),
            offline: false,
            frozen: false,
            locked: false,
            root_certificate: None,
            accept_invalid_certs: false,
            minify: Minify::Never,
            no_sri: false,
            allow_self_closing_script: false,
            create_nonce: None,
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

    /// Build [`HttpClientOptions`] options form configuration.
    pub fn client_options(&self) -> HttpClientOptions {
        HttpClientOptions {
            #[cfg(any(feature = "native-tls", feature = "rustls"))]
            root_certificate: self.root_certificate.clone(),
            #[cfg(any(feature = "native-tls", feature = "rustls"))]
            accept_invalid_certificates: self.accept_invalid_certs,
        }
    }
}

impl RtcBuilder for RtcBuild {
    type Options = BuildOptions;

    async fn build(configuration: Configuration, options: Self::Options) -> anyhow::Result<Self> {
        Self::new(configuration, options)
    }
}
