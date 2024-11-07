use crate::config::{CargoMetadata, Tools};
use anyhow::{anyhow, Context};
use cargo_lock::Lockfile;
use semver::{Comparator, Op, Prerelease, Version};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::str::FromStr;

/// Determines the value of `--target` flag for wasm-bindgen. For more details see
/// [here](https://rustwasm.github.io/wasm-bindgen/reference/deployment.html).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WasmBindgenTarget {
    Bundler,
    Web,
    NoModules,
    NodeJs,
    Deno,
}

impl FromStr for WasmBindgenTarget {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bundler" => Self::Bundler,
            "web" => Self::Web,
            "no-modules" => Self::NoModules,
            "nodejs" => Self::NodeJs,
            "deno" => Self::Deno,
            s => {
                return Err(anyhow!(
                    r#"unknown `data-bindgen-target="{s}"` value for <link data-trunk rel="rust" .../> attr; please ensure the value is lowercase and is a supported type"#
                ))
            }
        })
    }
}

impl Display for WasmBindgenTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bundler => f.write_str("bundler"),
            Self::Web => f.write_str("web"),
            Self::NoModules => f.write_str("no-modules"),
            Self::NodeJs => f.write_str("nodejs"),
            Self::Deno => f.write_str("deno"),
        }
    }
}

/// Find the appropriate version of `wasm-bindgen` to use. The version can be found in 3 different
/// locations in the order:
/// - Defined in the `Trunk.toml` as the highest priority.
/// - Located in the `Cargo.lock` if it exists. This is mostly the case as we run `cargo build`
///   before even calling this function.
/// - Located in the `Cargo.toml` as direct dependency of the project.
pub fn find_wasm_bindgen_version<'a>(
    cfg: &'a Tools,
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

/// Features supported by a certain version of wasm-bindgen.
pub struct WasmBindgenFeatures {
    /// Whether we can and should pass an object to the initialization function.
    ///
    /// In wasm-bindgen 0.2.93, parameters to `init` were deprecated in favor of
    /// an object (see [wasm-bindgen#3995]). From this version onward, wrap the
    /// arguments in an object and pass the object instead.
    ///
    /// [wasm-bindgen#3995]: https://github.com/rustwasm/wasm-bindgen/pull/3995
    pub init_with_object: bool,
}

const VERSION_GE_0_2_93: Comparator = Comparator {
    op: Op::GreaterEq,
    major: 0,
    minor: Some(2),
    patch: Some(93),
    pre: Prerelease::EMPTY,
};

impl WasmBindgenFeatures {
    pub fn from_version(version: &str) -> Result<Self, anyhow::Error> {
        let version = Version::parse(version).context("error parsing wasm-bindgen version")?;
        Ok(Self {
            init_with_object: VERSION_GE_0_2_93.matches(&version),
        })
    }
}
