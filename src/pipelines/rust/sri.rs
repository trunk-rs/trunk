use crate::common::html_rewrite::Document;
use crate::config::CrossOrigin;
use crate::processing::integrity::{IntegrityType, OutputDigest};
use anyhow::Context;
use std::collections::BTreeMap;
use std::fmt::{Display, Formatter};
use std::future::Future;
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub enum SriType {
    Preload,
    ModulePreload,
}

impl Display for SriType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Preload => f.write_str("preload"),
            Self::ModulePreload => f.write_str("modulepreload"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SriBuilder {
    r#type: IntegrityType,
    result: SriResult,
}

impl SriBuilder {
    pub fn new(r#type: IntegrityType) -> Self {
        Self {
            r#type,
            result: Default::default(),
        }
    }

    pub fn build(self) -> SriResult {
        self.result
    }

    /// Record the content of a file for SRI
    pub async fn record_file(
        &mut self,
        r#type: SriType,
        name: impl Into<String>,
        options: SriOptions,
        path: impl AsRef<Path>,
    ) -> anyhow::Result<()> {
        Ok(self
            .record(r#type, name, options, || async {
                tokio::fs::read(path).await
            })
            .await?)
    }

    /// Record content for SRI
    pub async fn record<F, T, E, Fut>(
        &mut self,
        r#type: SriType,
        name: impl Into<String>,
        options: SriOptions,
        source: F,
    ) -> Result<(), E>
    where
        F: FnOnce() -> Fut,
        T: AsRef<[u8]>,
        Fut: Future<Output = Result<T, E>>,
    {
        if !matches!(self.r#type, IntegrityType::None) {
            let name = name.into();
            let digest = OutputDigest::generate_async(self.r#type, source).await?;
            tracing::debug!(
                "recording SRI record - type: {:?}. name: {name}, value: {digest:?}",
                self.r#type,
            );
            self.result
                .integrities
                .insert((r#type, name), SriEntry { digest, options });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct SriResult {
    pub integrities: BTreeMap<(SriType, String), SriEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct SriOptions {
    pub r#as: Option<String>,
    pub r#type: Option<String>,
}

impl SriOptions {
    pub fn r#as(mut self, r#as: impl Into<String>) -> Self {
        self.r#as = Some(r#as.into());
        self
    }

    pub fn r#type(mut self, r#type: impl Into<String>) -> Self {
        self.r#type = Some(r#type.into());
        self
    }
}

impl Display for SriOptions {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let Some(r#as) = &self.r#as {
            write!(f, r#" as="{as}""#)?;
        }

        if let Some(r#type) = &self.r#type {
            write!(f, r#" type="{type}""#)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct SriEntry {
    pub digest: OutputDigest,
    pub options: SriOptions,
}

impl SriResult {
    pub fn inject(
        &self,
        location: &mut Document,
        head: &str,
        base: impl Display,
        cross_origin: CrossOrigin,
    ) -> anyhow::Result<()> {
        for ((r#type, name), SriEntry { digest, options }) in &self.integrities {
            if let Some(integrity) = digest.to_integrity_value() {
                let preload = format!(
                    r#"
<link rel="{type}" href="{base}{name}" crossorigin={cross_origin} integrity="{integrity}"{options}>"#,
                );
                location
                    .append_html(head, &preload)
                    .context("Unable to write SRI.")?;
            }
        }

        Ok(())
    }
}
