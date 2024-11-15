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

    /// The name of the output HTML file.
    #[arg(long, env = "TRUNK_BUILD_HTML_OUTPUT")]
    pub html_output: Option<String>,

    /// Build in release mode
    #[arg(long, env = "TRUNK_BUILD_RELEASE")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub release: Option<bool>,

    /// Cargo profile to use for building.
    #[arg(long, env = "TRUNK_BUILD_CARGO_PROFILE")]
    pub cargo_profile: Option<String>,

    /// The output dir for all final assets
    #[arg(short, long, env = "TRUNK_BUILD_DIST")]
    pub dist: Option<PathBuf>,

    #[arg(from_global)]
    pub offline: Option<bool>,

    /// Require Cargo.lock and cache are up to date
    #[arg(long, env = "TRUNK_BUILD_FROZEN")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub frozen: Option<bool>,

    /// Require Cargo.lock is up to date
    #[arg(long, env = "TRUNK_BUILD_LOCKED")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub locked: Option<bool>,

    /// The public URL from which assets are to be served
    #[arg(long, env = "TRUNK_BUILD_PUBLIC_URL")]
    pub public_url: Option<BaseUrl>,

    /// Don't add a trailing slash to the public URL if it is missing
    #[arg(long, env = "TRUNK_BUILD_PUBLIC_URL_NO_TRAILING_SLASH")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub public_url_no_trailing_slash_fix: Option<bool>,

    /// Build without default features
    #[arg(long, env = "TRUNK_BUILD_NO_DEFAULT_FEATURES")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub no_default_features: Option<bool>,

    /// Build with all features
    #[arg(long, env = "TRUNK_BUILD_ALL_FEATURES")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub all_features: Option<bool>,

    /// A comma-separated list of features to activate, must not be used with all-features
    #[arg(
        long,
        conflicts_with = "all_features",
        value_delimiter = ',',
        env = "TRUNK_BUILD_FEATURES"
    )]
    pub features: Option<Vec<String>>,

    /// Whether to include hash values in the output file names
    #[arg(long, env = "TRUNK_BUILD_FILEHASH")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub filehash: Option<bool>,

    /// Which example to build
    #[arg(long, env = "TRUNK_BUILD_EXAMPLE")]
    pub example: Option<String>,

    /// When desired, set a custom root certificate chain (same format as Cargo's config.toml http.cainfo)
    #[arg(long, env = "TRUNK_BUILD_ROOT_CERTIFICATE")]
    pub root_certificate: Option<String>,

    /// Allows request to ignore certificate validation errors (danger!)
    ///
    /// Can be useful when behind a corporate proxy.
    #[arg(long, env = "TRUNK_BUILD_ACCEPT_INVALID_CERTS")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub accept_invalid_certs: Option<bool>,

    /// Enable minification.
    ///
    /// This overrides the value from the configuration file.
    #[arg(short = 'M', long, env = "TRUNK_BUILD_MINIFY")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub minify: Option<bool>,

    /// Allows disabling sub-resource integrity (SRI)
    #[arg(long, env = "TRUNK_BUILD_NO_SRI")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub no_sri: Option<bool>,

    /// Ignore error's related to self-closing script elements, and instead issue a warning.
    ///
    /// Since this issue can cause the HTML output to be truncated, only enable this in case you
    /// are sure it is caused due to a false positive.
    #[arg(long, env = "TRUNK_BUILD_ALLOW_SELF_CLOSING_SCRIPT")]
    #[arg(default_missing_value="true", num_args=0..=1)]
    pub allow_self_closing_script: Option<bool>,

    // NOTE: flattened structures come last
    #[command(flatten)]
    pub core: super::core::Core,

    #[command(flatten)]
    pub tools: Tools,
}

impl Build {
    /// apply CLI overrides to the configuration
    pub fn apply_to(self, mut config: Configuration) -> Result<Configuration> {
        let Self {
            core,
            target,
            html_output,
            release,
            cargo_profile,
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
            example,
            root_certificate,
            accept_invalid_certs,
            minify,
            no_sri,
            allow_self_closing_script,
            tools,
        } = self;

        config.build.target = target.unwrap_or(config.build.target);
        config.build.html_output = html_output.or(config.build.html_output);
        config.build.release = release.unwrap_or(config.build.release);
        config.build.cargo_profile = cargo_profile.or(config.build.cargo_profile);
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
        config.build.example = example.or(config.build.example);

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

#[cfg(test)]
mod test {
    use crate::{Trunk, TrunkSubcommands};
    use clap::Parser;
    use rstest::rstest;

    #[rstest]
    #[case(&["trunk", "build"], None)]
    #[case(&["trunk", "build", "--no-default-features"], Some(true))]
    #[case(&["trunk", "build", "--no-default-features", "true"], Some(true))]
    #[case(&["trunk", "build", "--no-default-features", "false"], Some(false))]
    fn test_bool_no_arg(#[case] input: &[&str], #[case] expected: Option<bool>) {
        let cli = Trunk::parse_from(input);
        let TrunkSubcommands::Build(build) = cli.action else {
            panic!("must be a build command");
        };

        assert_eq!(build.no_default_features, expected);
    }
}
