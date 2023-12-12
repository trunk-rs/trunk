use super::super::trunk_id_selector;
use crate::config::{CrossOrigin, RtcBuild};
use crate::pipelines::rust::{IntegrityOutput, RustAppType};
use crate::processing::integrity::OutputDigest;
use nipper::Document;
use std::collections::HashMap;
use std::sync::Arc;

/// The output of a cargo build pipeline.
pub struct RustAppOutput {
    /// The runtime build config.
    pub cfg: Arc<RtcBuild>,
    /// The ID of this pipeline.
    pub id: Option<usize>,
    /// The filename of the generated JS loader file written to the dist dir.
    pub js_output: String,
    /// The filename of the generated WASM file written to the dist dir.
    pub wasm_output: String,
    /// The filename of the generated .ts file written to the dist dir.
    pub ts_output: Option<String>,
    /// The filename of the generated loader shim script for web workers written to the dist dir.
    pub loader_shim_output: Option<String>,
    /// Is this module main or a worker.
    pub r#type: RustAppType,
    /// The cross-origin setting for loading the resources
    pub cross_origin: CrossOrigin,
    /// The integrity and digest of the output, ignored in case of [`super::IntegrityType::None`]
    pub integrity: IntegrityOutput,
    /// The output digests for the discovered snippets
    pub snippet_integrities: HashMap<String, OutputDigest>,
    /// Import functions exported from Rust into JavaScript
    pub import_bindings: bool,
    /// The name of the WASM bindings import
    pub import_bindings_name: Option<String>,
}

pub fn pattern_evaluate(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (k, v) in params.iter() {
        let pattern = format!("{{{}}}", k.as_str());
        if let Some(file_path) = v.strip_prefix('@') {
            if let Ok(contents) = std::fs::read_to_string(file_path) {
                result = str::replace(result.as_str(), &pattern, contents.as_str());
            }
        } else {
            result = str::replace(result.as_str(), &pattern, v);
        }
    }
    result
}

impl RustAppOutput {
    pub async fn finalize(self, dom: &mut Document) -> anyhow::Result<()> {
        if self.r#type == RustAppType::Worker {
            // Skip the script tag and preload links for workers, and remove the link tag only.
            // Workers are initialized and managed by the app itself at runtime.
            if let Some(id) = self.id {
                dom.select(&trunk_id_selector(id)).remove();
            }
            return Ok(());
        }

        if !self.cfg.inject_scripts {
            // Configuration directed we do not inject any scripts.
            return Ok(());
        }

        let (base, js, wasm, head, body) = (
            &self.cfg.public_url,
            &self.js_output,
            &self.wasm_output,
            "html head",
            "html body",
        );
        let (pattern_script, pattern_preload) =
            (&self.cfg.pattern_script, &self.cfg.pattern_preload);
        let mut params: HashMap<String, String> = match &self.cfg.pattern_params {
            Some(x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), base.clone());
        params.insert("js".to_owned(), js.clone());
        params.insert("wasm".to_owned(), wasm.clone());
        params.insert("crossorigin".to_owned(), self.cross_origin.to_string());

        let preload = match pattern_preload {
            Some(pattern) => pattern_evaluate(pattern, &params),
            None => {
                format!(
                    r#"
<link rel="preload" href="{base}{wasm}" as="fetch" type="application/wasm" crossorigin={cross_origin}{wasm_integrity}>
<link rel="modulepreload" href="{base}{js}" crossorigin={cross_origin}{js_integrity}>"#,
                    cross_origin = self.cross_origin,
                    wasm_integrity = self.integrity.wasm.make_attribute(),
                    js_integrity = self.integrity.js.make_attribute(),
                )
            }
        };
        dom.select(head).append_html(preload);

        for (name, integrity) in self.snippet_integrities {
            if let Some(integrity) = integrity.to_integrity_value() {
                let preload = format!(
                    r#"
<link rel="modulepreload" href="{base}{name}" crossorigin={cross_origin} integrity="{integrity}">"#,
                    cross_origin = self.cross_origin,
                );
                dom.select(head).append_html(preload);
            }
        }

        let script = match pattern_script {
            Some(pattern) => pattern_evaluate(pattern, &params),
            None => {
                let (import, bind) = match self.import_bindings {
                    true => (
                        ", * as bindings",
                        format!(
                            r#"
window.{bindings} = bindings;
"#,
                            bindings = self
                                .import_bindings_name
                                .as_deref()
                                .unwrap_or("wasmBindings")
                        ),
                    ),
                    false => ("", String::new()),
                };
                format!(
                    r#"
<script type="module">
import init{import} from '{base}{js}';
init('{base}{wasm}');{bind}
</script>"#,
                )
            }
        };

        match self.id {
            Some(id) => dom.select(&trunk_id_selector(id)).replace_with_html(script),
            None => dom.select(body).append_html(script),
        }
        Ok(())
    }
}
