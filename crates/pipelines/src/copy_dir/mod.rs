//! Copy-dir asset pipeline.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures_util::future::ok;
use futures_util::FutureExt;
use nipper::Document;
use tokio::fs;
use tokio::task::JoinHandle;

#[cfg(test)]
mod tests;

use crate::util::{
    copy_dir_recursive, trunk_id_selector, Attrs, ErrorReason, Result, ResultExt, ATTR_HREF,
};
use crate::{Output, Pipeline};

/// A trait that indicates a type can be used as config type for copy dir pipeline.
pub trait CopyDirConfig {
    /// Returns the directory where the output shoule write to.
    fn output_dir(&self) -> &Path;
}

/// A CopyDir asset pipeline.
pub struct CopyDir<C> {
    /// The ID of this pipeline's source HTML element.
    id: usize,
    /// Runtime build config.
    cfg: Arc<C>,
    /// The path to the dir being copied.
    path: PathBuf,
    /// Optional target path inside the dist dir.
    target_path: Option<PathBuf>,
}

impl<C> CopyDir<C>
where
    C: CopyDirConfig,
{
    pub const TYPE_COPY_DIR: &'static str = "copy-dir";

    pub async fn new(cfg: Arc<C>, html_dir: Arc<PathBuf>, attrs: Attrs, id: usize) -> Result<Self> {
        // Build the path to the target asset.
        let href_attr =
            attrs
                .get(ATTR_HREF)
                .with_reason(|| ErrorReason::PipelineLinkHrefNotFound {
                    rel: "copy-dir".into(),
                })?;
        let mut path = PathBuf::new();
        path.extend(href_attr.split('/'));
        if !path.is_absolute() {
            path = html_dir.join(path);
        }
        let target_path = attrs
            .get("data-target-path")
            .map(|m| Path::new(m).to_owned());

        Ok(Self {
            id,
            cfg,
            path,
            target_path,
        })
    }

    /// Run this pipeline.
    #[tracing::instrument(level = "trace", skip(self))]
    async fn run(self) -> Result<CopyDirOutput> {
        let rel_path = crate::util::strip_prefix(&self.path);
        tracing::info!(path = ?rel_path, "copying directory");

        let canonical_path =
            fs::canonicalize(&self.path)
                .await
                .with_reason(|| ErrorReason::FsNotExist {
                    path: self.path.to_owned(),
                })?;
        let dir_name = canonical_path
            .file_name()
            .with_reason(|| ErrorReason::PathNoFileStem {
                path: canonical_path.to_owned(),
            })?;

        let out_rel_path = self
            .target_path
            .as_deref()
            .unwrap_or_else(|| dir_name.as_ref());

        let dir_out = self.cfg.output_dir().join(out_rel_path);

        if !dir_out.starts_with(self.cfg.output_dir()) {
            return Err(ErrorReason::PipelineLinkDataTargetPathRelativeExpected {
                path: out_rel_path.to_owned(),
            }
            .into_error());
        }

        copy_dir_recursive(canonical_path, dir_out).await?;

        tracing::info!(path = ?rel_path, "finished copying directory");
        Ok(CopyDirOutput(self.id))
    }
}

impl<C> Pipeline for CopyDir<C>
where
    C: 'static + CopyDirConfig + Send + Sync,
{
    type Output = CopyDirOutput;

    #[tracing::instrument(level = "trace", skip(self))]
    fn spawn(self) -> JoinHandle<Result<CopyDirOutput>> {
        tokio::spawn(self.run())
    }
}

/// The output of a CopyDir build pipeline.
pub struct CopyDirOutput(usize);

impl Output for CopyDirOutput {
    fn finalize<'life0, 'async_trait>(
        self,
        dom: &'life0 mut Document,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = Result<()>> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        Self: 'async_trait,
    {
        dom.select(&trunk_id_selector(self.0)).remove();
        ok(()).boxed()
    }
}
