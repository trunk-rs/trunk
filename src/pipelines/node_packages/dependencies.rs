//! Discovery of node packages declared by the crate and its Rust dependencies.
//!
//! A crate can declare node packages (and the trunk asset links they provide) in its
//! `Cargo.toml`:
//!
//! ```toml
//! [[package.metadata.trunk.node_packages]]
//! name = "@patternfly/patternfly"
//! version = "6.4.0"
//! links = [
//!   { rel = "sass", href = "patternfly.scss" },
//!   { rel = "copy-dir", href = "assets", data-target-path = "assets/" },
//! ]
//! ```
//!
//! Every crate depending on it (directly or transitively) gets those packages installed and the
//! links injected into its HTML document, without having to declare them again.

use crate::{
    config::{
        models::NodePackage,
        rt::{Features, RtcBuild},
    },
    pipelines::{ATTR_HREF, ATTR_REL},
};
use anyhow::{Context, Result};
use cargo_metadata::{CargoOpt, DependencyKind, Metadata, MetadataCommand, PackageId};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};
use tokio::task::spawn_blocking;

/// The content of `[package.metadata.trunk]`, only the parts relevant here.
#[derive(Debug, Default, Deserialize)]
struct TrunkPackageMetadata {
    #[serde(default)]
    node_packages: Vec<NodePackage>,
}

/// Collect all node packages: those declared by the Rust crate graph (dependencies first),
/// followed by the ones of the trunk configuration.
///
/// The `manifest` is the path to the `Cargo.toml` of the application. If it does not exist,
/// only the packages of the configuration are returned.
pub async fn collect_node_packages(cfg: &RtcBuild, manifest: &Path) -> Vec<NodePackage> {
    let mut packages = Vec::new();

    if manifest.is_file() {
        // Discovery is best effort: failing here must not break builds which worked before.
        match cargo_metadata(cfg, manifest).await {
            Ok(metadata) => packages.extend(from_metadata(&metadata)),
            Err(err) => tracing::warn!("unable to discover node packages of dependencies: {err:#}"),
        }
    } else {
        tracing::debug!(
            "no cargo manifest found at {}, skipping node packages of dependencies",
            manifest.display()
        );
    }

    packages.extend(cfg.node_packages.iter().cloned());

    merge(packages)
}

async fn cargo_metadata(cfg: &RtcBuild, manifest: &Path) -> Result<Metadata> {
    let mut cmd = MetadataCommand::new();
    cmd.manifest_path(dunce::simplified(manifest));

    match &cfg.cargo_features {
        Features::All => {
            cmd.features(CargoOpt::AllFeatures);
        }
        Features::Custom {
            features,
            no_default_features,
        } => {
            if *no_default_features {
                cmd.features(CargoOpt::NoDefaultFeatures);
            }
            if let Some(features) = features {
                cmd.features(CargoOpt::SomeFeatures(
                    features.split([',', ' ']).map(str::to_string).collect(),
                ));
            }
        }
    }

    let mut other = Vec::new();
    if cfg.offline {
        other.push("--offline".to_string());
    }
    if cfg.frozen {
        other.push("--frozen".to_string());
    }
    if cfg.locked {
        other.push("--locked".to_string());
    }
    cmd.other_options(other);

    spawn_blocking(move || cmd.exec())
        .await
        .context("error awaiting spawned cargo metadata task")?
        .context("error getting cargo metadata to discover node packages of dependencies")
}

/// Extract node packages from the resolved crate graph.
///
/// Only normal (non-dev, non-build) dependencies reachable from the root package are considered.
/// Dependencies come before their dependents, so that the links of a dependent can override
/// the styles of a dependency.
fn from_metadata(metadata: &Metadata) -> Vec<NodePackage> {
    let Some(resolve) = &metadata.resolve else {
        return vec![];
    };
    let Some(root) = &resolve.root else {
        return vec![];
    };

    let nodes: HashMap<&PackageId, _> = resolve.nodes.iter().map(|n| (&n.id, n)).collect();
    let packages: HashMap<&PackageId, _> = metadata.packages.iter().map(|p| (&p.id, p)).collect();

    // iterative post-order DFS
    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut stack = vec![(root, false)];
    while let Some((id, expanded)) = stack.pop() {
        if expanded {
            order.push(id);
            continue;
        }
        if !visited.insert(id) {
            continue;
        }
        stack.push((id, true));
        if let Some(node) = nodes.get(id) {
            for dep in node.deps.iter().rev() {
                let normal = dep
                    .dep_kinds
                    .iter()
                    .any(|kind| kind.kind == DependencyKind::Normal);
                if normal && !visited.contains(&dep.pkg) {
                    stack.push((&dep.pkg, false));
                }
            }
        }
    }

    let mut result = Vec::new();
    for id in order {
        let Some(package) = packages.get(id) else {
            continue;
        };
        let Some(trunk) = package.metadata.get("trunk") else {
            continue;
        };
        let trunk = match TrunkPackageMetadata::deserialize(trunk) {
            Ok(trunk) => trunk,
            Err(err) => {
                tracing::warn!(
                    "ignoring invalid [package.metadata.trunk] section in {}: {err}",
                    package.manifest_path
                );
                continue;
            }
        };
        if !trunk.node_packages.is_empty() {
            tracing::debug!(
                "crate {} declares node packages: {:?}",
                package.name,
                trunk.node_packages
            );
        }
        result.extend(trunk.node_packages);
    }

    result
}

/// Merge entries referring to the same package installation, keeping the first position and
/// collecting all distinct links.
fn merge(packages: Vec<NodePackage>) -> Vec<NodePackage> {
    let mut result: Vec<NodePackage> = Vec::new();
    for package in packages {
        match result.iter_mut().find(|p| p.same_installation(&package)) {
            Some(existing) => {
                for link in package.links {
                    if !existing.links.contains(&link) {
                        existing.links.push(link);
                    }
                }
            }
            None => result.push(package),
        }
    }
    result
}

/// Render the `<link data-trunk ...>` elements of all node packages.
///
/// The `href` of each link is resolved against the installation directory of its package.
///
/// Attribute values are written verbatim, as trunk reads them back without decoding HTML
/// entities. Links with values or names which can't be represented safely are skipped.
pub fn render_node_package_links(working_directory: &Path, packages: &[NodePackage]) -> String {
    fn valid_name(name: &str) -> bool {
        !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
    }
    fn valid_value(value: &str) -> bool {
        !value.contains(['"', '<', '>'])
    }

    let mut html = String::new();
    for package in packages {
        let base = working_directory.join(package.install_path());
        for link in &package.links {
            let href = base.join(&link.href).to_string_lossy().to_string();
            let mut attrs = vec![(ATTR_REL, link.rel.as_str()), (ATTR_HREF, href.as_str())];
            attrs.extend(
                link.attributes
                    .iter()
                    .filter(|(name, _)| !matches!(name.as_str(), ATTR_REL | ATTR_HREF))
                    .map(|(name, value)| (name.as_str(), value.as_str())),
            );

            if !attrs
                .iter()
                .all(|(name, value)| valid_name(name) && valid_value(value))
            {
                tracing::warn!(
                    "skipping invalid link {link:?} of node package {}@{}",
                    package.name,
                    package.version
                );
                continue;
            }

            html.push_str("<link data-trunk");
            for (name, value) in attrs {
                html.push_str(&format!(r#" {name}="{value}""#));
            }
            html.push_str(">\n");
        }
    }
    html
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::config::models::NodePackageLink;

    fn package(name: &str, links: &[(&str, &str)]) -> NodePackage {
        NodePackage {
            registry: None,
            name: name.into(),
            version: "1.0.0".into(),
            target_path: None,
            links: links
                .iter()
                .map(|(rel, href)| NodePackageLink {
                    rel: rel.to_string(),
                    href: href.to_string(),
                    attributes: Default::default(),
                })
                .collect(),
        }
    }

    #[test]
    fn merge_dedupes_packages_and_links() {
        let merged = merge(vec![
            package("a", &[("sass", "a.scss")]),
            package("b", &[]),
            package("a", &[("sass", "a.scss"), ("copy-dir", "assets")]),
        ]);
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].name, "a");
        assert_eq!(merged[0].links.len(), 2);
        assert_eq!(merged[1].name, "b");
    }

    #[test]
    fn render_links() {
        let mut pkg = package("@scope/pkg", &[("copy-dir", "assets")]);
        pkg.links[0]
            .attributes
            .insert("data-target-path".into(), "assets/".into());
        let html = render_node_package_links(Path::new("/work"), &[pkg]);
        let expected_href = Path::new("/work")
            .join("target/node_modules/@scope/pkg/1.0.0")
            .join("assets");
        assert_eq!(
            html,
            format!(
                "<link data-trunk rel=\"copy-dir\" href=\"{}\" data-target-path=\"assets/\">\n",
                expected_href.display()
            )
        );

        let mut invalid = package("pkg", &[("sass", "a\".scss")]);
        invalid.links.push(NodePackageLink {
            rel: "sass".into(),
            href: "ok.scss".into(),
            attributes: [("on load".to_string(), "x".to_string())].into(),
        });
        assert_eq!(
            render_node_package_links(Path::new("/work"), &[invalid]),
            ""
        );
    }

    #[test]
    fn parse_package_metadata() {
        let value = serde_json::json!({
            "node_packages": [{
                "name": "@patternfly/patternfly",
                "version": "6.4.0",
                "links": [
                    { "rel": "sass", "href": "patternfly.scss" },
                    { "rel": "copy-dir", "href": "assets", "data-target-path": "assets/" }
                ]
            }]
        });
        let metadata = TrunkPackageMetadata::deserialize(&value).expect("valid metadata");
        let links = &metadata.node_packages[0].links;
        assert_eq!(links.len(), 2);
        assert_eq!(links[1].attributes["data-target-path"], "assets/");
    }
}
