mod dependencies;
mod get_package_error;
mod node_package_client;
mod node_package_information;

pub use dependencies::{collect_node_packages, render_node_package_links};

use crate::{
    common::copy_dir_recursive, config::models::NodePackage,
    pipelines::node_packages::node_package_client::NodePackageClient,
};
use anyhow::Context;
use async_compression::tokio::bufread::GzipDecoder;
use futures_util::{StreamExt, TryStreamExt, stream::FuturesUnordered};
use std::{io, path::Path};
use tokio::{fs::remove_dir_all, task::JoinHandle};
use tokio_util::io::StreamReader;

/// A `FuturesUnordered` containing a `JoinHandle` for each hook-running task.
pub type NodePackageHandles = FuturesUnordered<JoinHandle<anyhow::Result<()>>>;
pub fn spawn_node_packages(
    working_directory: &Path,
    node_packages: &[NodePackage],
) -> NodePackageHandles {
    tracing::debug!("node packages {node_packages:?}");

    let futures: FuturesUnordered<_> = node_packages
        .iter()
        .map(|node_package_cfg| {
            let package_information = format!(
                "{}@{}{}",
                node_package_cfg.name,
                node_package_cfg.version,
                node_package_cfg
                    .registry
                    .clone()
                    .map(|registry| format!("(registry: {registry})"))
                    .unwrap_or_default()
            );

            let node_package_cfg = node_package_cfg.clone();
            let target_path = working_directory.join(node_package_cfg.install_path());

            tokio::spawn(async move {
                let http_node_module_client = if let Some(registry) = node_package_cfg.registry {
                    NodePackageClient::new(&registry)?
                } else {
                    NodePackageClient::default()
                };

                if target_path.exists() {
                    tracing::debug!(
                        "target path ({}) already exists, skipping",
                        target_path.display()
                    );
                    return Ok(());
                }

                tracing::info!("download node package {package_information}");

                let package = http_node_module_client
                    .get_package(&node_package_cfg.name, &node_package_cfg.version)
                    .await
                    .with_context(|| {
                        format!("failed to retrieve Node package: {package_information}")
                    })?;

                // request

                let tarball = reqwest::get(package.distribution.tarball)
                    .await?
                    .bytes_stream();

                // unpack

                let tarball = tarball.map_err(io::Error::other);
                let archive_data = GzipDecoder::new(StreamReader::new(tarball));
                let archive = async_tar::Archive::new(archive_data);
                archive.unpack(&target_path).await?;

                // move

                let package_directory = target_path.join("package");

                tracing::debug!("move from {package_directory:?} to {target_path:?}");

                copy_dir_recursive(package_directory.clone(), target_path).await?;
                remove_dir_all(package_directory).await?;

                // done

                tracing::info!("finished to download node package {package_information}");

                Ok(())
            })
        })
        .collect();

    futures
}

/// Waits for all the given hooks to finish.
pub async fn wait_node_packages(mut futures: NodePackageHandles) -> anyhow::Result<()> {
    while let Some(result) = futures.next().await {
        result??;
    }

    Ok(())
}
