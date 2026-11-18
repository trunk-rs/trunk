mod get_package_error;
mod integrity;
mod node_package_client;
mod node_package_information;

use crate::{
    common::copy_dir_recursive,
    config::rt::RtcBuild,
    pipelines::node_packages::{integrity::Integrity, node_package_client::NodePackageClient},
};
use anyhow::Context;
use async_compression::tokio::bufread::GzipDecoder;
use futures_util::{StreamExt, stream::FuturesUnordered};
use std::{io, path::Path, sync::Arc};
use tokio::{fs::remove_dir_all, io::AsyncRead, task::JoinHandle};

/// A `FuturesUnordered` containing a `JoinHandle` for each hook-running task.
pub type NodePackageHandles = FuturesUnordered<JoinHandle<anyhow::Result<()>>>;
pub fn spawn_node_packages(cfg: Arc<RtcBuild>) -> NodePackageHandles {
    tracing::info!("node packages {:?}", cfg.node_packages);
    let working_directory = cfg.working_directory.clone();

    let futures: FuturesUnordered<_> = cfg
        .node_packages
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
            let working_directory = working_directory.clone();

            tokio::spawn(async move {
                let http_node_module_client = if let Some(registry) = node_package_cfg.registry {
                    NodePackageClient::new(&registry)?
                } else {
                    NodePackageClient::default()
                };

                let target_path = node_package_cfg.target_path.unwrap_or(format!(
                    "target/node_modules/{}/{}",
                    node_package_cfg.name, node_package_cfg.version
                ));
                let target_path = working_directory.join(target_path);

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

                let integrity = Integrity::from_distribution(
                    package.distribution.integrity.as_deref(),
                    package.distribution.shasum.as_deref(),
                )
                .with_context(|| format!("unable to verify Node package: {package_information}"))?;

                // request

                let tarball = http_node_module_client
                    .download_tarball(&package.distribution.tarball)
                    .await
                    .with_context(|| {
                        format!("failed to download Node package: {package_information}")
                    })?;

                // verify, before anything is written to disk

                integrity.verify(&tarball).with_context(|| {
                    format!("failed to verify Node package: {package_information}")
                })?;

                // unpack

                let archive_data = GzipDecoder::new(&tarball[..]);
                unpack_without_links(archive_data, &target_path).await?;

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

/// Unpacks a package tarball into `target`, skipping symbolic and hard links.
///
/// This is what npm does as well (see `#tarxOptions` in pacote's `lib/fetcher.js`), so no
/// package can rely on links. Links would otherwise allow a package to make trunk read or write
/// files outside of `target`: while unpacking, by applying the permissions of a directory entry
/// through a link, and afterwards, when the package content is moved out of `package/`.
async fn unpack_without_links<R>(archive_data: R, target: &Path) -> io::Result<()>
where
    R: AsyncRead + Unpin + Send,
{
    tokio::fs::create_dir_all(target).await?;
    // As `async_tar::Archive::unpack` does, so paths can exceed 260 characters on Windows.
    let target = tokio::fs::canonicalize(target).await?;

    let mut entries = async_tar::Archive::new(archive_data).entries()?;
    // As `async_tar::Archive::unpack` does, create directories last so that their
    // permissions do not prevent unpacking their content.
    let mut directories = Vec::new();
    while let Some(entry) = entries.next().await {
        let mut entry = entry?;
        let entry_type = entry.header().entry_type();
        if entry_type.is_symlink() || entry_type.is_hard_link() {
            tracing::warn!(
                "skipping link {} in node package, as npm does",
                entry.path()?.display()
            );
        } else if entry_type.is_dir() {
            directories.push(entry);
        } else {
            entry.unpack_in(&target).await?;
        }
    }
    for mut directory in directories {
        directory.unpack_in(&target).await?;
    }

    Ok(())
}

/// Waits for all the given hooks to finish.
pub async fn wait_node_packages(mut futures: NodePackageHandles) -> anyhow::Result<()> {
    while let Some(result) = futures.next().await {
        result??;
    }

    Ok(())
}
