mod get_package_error;
mod node_package_client;
mod node_package_information;

use crate::common::copy_dir_recursive;
use crate::config::rt::RtcBuild;
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use futures_util::io::AllowStdIo;
use futures_util::stream::FuturesUnordered;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// A `FuturesUnordered` containing a `JoinHandle` for each hook-running task.
pub type NodePackageHandles = FuturesUnordered<JoinHandle<anyhow::Result<()>>>;
pub fn spawn_node_packages(cfg: Arc<RtcBuild>) -> NodePackageHandles {
    tracing::info!("node packages {:?}", cfg.node_packages);

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

            tracing::info!("download node package {package_information}");

            let node_package_cfg = node_package_cfg.clone();

            tokio::spawn(async move {
                let http_node_module_client = if let Some(registry) = node_package_cfg.registry {
                    node_package_client::NodePackageClient::new(&registry)?
                } else {
                    node_package_client::NodePackageClient::default()
                };

                let target_path = node_package_cfg.target_path.unwrap_or(format!(
                    "target/node_modules/{}/{}",
                    node_package_cfg.name, node_package_cfg.version
                ));
                let target_path = PathBuf::from(target_path);

                if !target_path.exists()
                    && let Ok(package) = http_node_module_client
                        .get_package(&node_package_cfg.name, &node_package_cfg.version)
                        .await
                {
                    let tarball = reqwest::get(package.distribution.tarball)
                        .await?
                        .bytes()
                        .await?;

                    let archive_data =
                        AllowStdIo::new(GzDecoder::new(std::io::Cursor::new(tarball)));

                    let archive = async_tar::Archive::new(archive_data);

                    archive.unpack(&target_path).await?;

                    let package_directory = target_path.join("package");

                    tracing::debug!("move from {package_directory:?} to {target_path:?}");

                    copy_dir_recursive(package_directory.clone(), target_path).await?;
                    std::fs::remove_dir_all(package_directory)?;

                    tracing::info!("finished to download node package {package_information}");
                }

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
