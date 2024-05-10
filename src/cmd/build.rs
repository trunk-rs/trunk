use crate::{
    build::BuildSystem,
    config::{
        self,
        rt::{self, RtcBuild, RtcBuilder},
        types::{BaseUrl, Minify},
        Configuration, Tools,
    },
};
use anyhow::Result;
use clap::Args;
use std::{path::PathBuf, sync::Arc};

/// Build the Rust WASM app and all of its assets.
#[derive(Clone, Debug, Args)]
#[command(name = "build")]
#[command(next_help_heading = "Build")]
pub struct Build {
    /// The index HTML file to drive the bundling process
    pub target: Option<PathBuf>,

    /// Build in release mode
    #[arg(long)]
    pub release: Option<bool>,

    /// The output dir for all final assets
    #[arg(short, long)]
    pub dist: Option<PathBuf>,

    /// Run without accessing the network
    #[arg(long)]
    pub offline: Option<bool>,

    /// Require Cargo.lock and cache are up to date
    #[arg(long)]
    pub frozen: Option<bool>,

    /// Require Cargo.lock is up to date
    #[arg(long)]
    pub locked: Option<bool>,

    /// The public URL from which assets are to be served
    #[arg(long)]
    pub public_url: Option<BaseUrl>,

    /// Don't add a trailing slash to the public URL if it is missing
    #[arg(long)]
    pub public_url_no_trailing_slash_fix: Option<bool>,

    /// Build without default features
    #[arg(long)]
    pub no_default_features: Option<bool>,

    /// Build with all features
    #[arg(long)]
    pub all_features: Option<bool>,

    /// A comma-separated list of features to activate, must not be used with all-features
    #[arg(long, conflicts_with = "all_features", value_delimiter = ',')]
    pub features: Option<Vec<String>>,

    /// Whether to include hash values in the output file names
    #[arg(long)]
    pub filehash: Option<bool>,

    /// When desired, set a custom root certificate chain (same format as Cargo's config.toml http.cainfo)
    #[arg(long)]
    pub root_certificate: Option<String>,

    /// Allows request to ignore certificate validation errors.
    ///
    /// Can be useful when behind a corporate proxy.
    #[arg(long)]
    pub accept_invalid_certs: Option<bool>,

    /// Enable minification.
    ///
    /// This overrides the value from the configuration file.
    #[arg(short = 'M', long)]
    pub minify: Option<bool>,

    /// Allows disabling sub-resource integrity (SRI)
    #[arg(long)]
    pub no_sri: Option<bool>,

    /// Ignore error's related to self-closing script elements, and instead issue a warning.
    ///
    /// Since this issue can cause the HTML output to be truncated, only enable this in case you
    /// are sure it is caused due to a false positive.
    #[arg(long)]
    pub allow_self_closing_script: Option<bool>,

    #[command(flatten)]
    pub core: super::core::Core,

    // NOTE: flattened structures come last
    #[command(flatten)]
    pub tools: Tools,
}

impl Build {
    /// apply CLI overrides to the configuration
    pub fn apply_to(self, mut config: Configuration) -> Result<Configuration> {
        let Self {
            core,
            target,
            release,
            dist,
            offline,
            frozen,
            locked,
            public_url,
            public_url_no_trailing_slash_fix,
            no_default_features,
            all_features,
            features,
            filehash,
            root_certificate,
            accept_invalid_certs,
            minify,
            no_sri,
            allow_self_closing_script,
            tools,
        } = self;

        config.build.target = target.unwrap_or(config.build.target);
        config.build.release = release.unwrap_or(config.build.release);
        config.build.dist = dist.unwrap_or(config.build.dist);
        config.build.offline = offline.unwrap_or(config.build.offline);
        config.build.frozen = frozen.unwrap_or(config.build.frozen);
        config.build.locked = locked.unwrap_or(config.build.locked);
        config.build.public_url = public_url.unwrap_or(config.build.public_url);
        config.build.public_url_no_trailing_slash_fix = public_url_no_trailing_slash_fix
            .unwrap_or(config.build.public_url_no_trailing_slash_fix);

        config.build.no_default_features =
            no_default_features.unwrap_or(config.build.no_default_features);
        config.build.all_features = all_features.unwrap_or(config.build.all_features);
        config.build.features = features.unwrap_or(config.build.features);

        config.build.filehash = filehash.unwrap_or(config.build.filehash);

        config.build.root_certificate = root_certificate.or(config.build.root_certificate);
        config.build.accept_invalid_certs =
            accept_invalid_certs.unwrap_or(config.build.accept_invalid_certs);
        config.build.minify = minify
            .map(|minify| match minify {
                true => Minify::Always,
                false => Minify::Never,
            })
            .unwrap_or(config.build.minify);
        config.build.no_sri = no_sri.unwrap_or(config.build.no_sri);
        config.build.allow_self_closing_script =
            allow_self_closing_script.unwrap_or(config.build.allow_self_closing_script);

        let config = core.apply_to(config)?;
        let config = tools.apply_to(config)?;

        Ok(config)
    }

    #[tracing::instrument(level = "trace", skip(self, config))]
    pub async fn run(self, config: Option<PathBuf>) -> Result<()> {
        let (cfg, working_directory) = config::load(config).await?;

        let cfg = self.apply_to(cfg)?;
        let cfg = RtcBuild::from_config(cfg, working_directory, |_, core| rt::BuildOptions {
            core,
            inject_autoloader: false,
        })
        .await?;

        cfg.core.enforce_version()?;

        let mut system = BuildSystem::new(Arc::new(cfg), None, None).await?;
        system.build().await?;
        Ok(())
    }
}
