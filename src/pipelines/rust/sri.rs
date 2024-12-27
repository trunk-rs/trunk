use crate::common::nonce_attr;
use crate::config::types::CrossOrigin;
use crate::{
    common::html_rewrite::Document,
    processing::integrity::{IntegrityType, OutputDigest},
};
use anyhow::Context;
use std::{
    fmt::{Display, Formatter},
    future::Future,
    path::Path,
};

#[derive(Clone, Debug, PartialEq, Eq)]
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
        let name = name.into();
        let digest = match self.r#type {
            IntegrityType::None => OutputDigest::default(),
            _ => OutputDigest::generate_async(self.r#type, source).await?,
        };
        tracing::debug!(
            "recording SRI record - type: {:?}. name: {name}, value: {digest:?}",
            self.r#type,
        );
        let key = SriKey { r#type, name };
        let entry = SriEntry { digest, options };
        if let Some(record) = self.result.integrities.iter_mut().find(|(k, _)| k == &key) {
            record.1 = entry;
        } else {
            self.result.integrities.push((key, entry));
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct SriResult {
    pub integrities: Vec<(SriKey, SriEntry)>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SriKey {
    pub r#type: SriType,
    pub name: String,
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
        create_nonce: &Option<String>,
    ) -> anyhow::Result<()> {
        let nonce = nonce_attr(create_nonce);
        for (SriKey { r#type, name }, SriEntry { digest, options }) in &self.integrities {
            let preload = if let Some(integrity) = digest.to_integrity_value() {
                format!(
                    r#"<link rel="{type}"{nonce} href="{base}{name}" crossorigin="{cross_origin}" integrity="{integrity}"{options}>"#,
                )
            } else {
                format!(
                    r#"<link rel="{type}"{nonce} href="{base}{name}" crossorigin="{cross_origin}"{options}>"#,
                )
            };
            location
                .append_html(head, &preload)
                .context("Unable to write SRI.")?;
        }

        Ok(())
    }
}
