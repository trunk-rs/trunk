use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use trunk_pipelines::assets::{
    CopyDirConfig, CopyFileConfig, CssConfig, IconConfig, JsConfig, RustAppConfig, SassConfig,
    TailwindCssConfig,
};
use trunk_pipelines::html::HtmlPipelineConfig;
pub(crate) use trunk_pipelines::html::{HtmlPipeline, PipelineStage};

use crate::config::RtcBuild;

impl JsConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl CssConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl SassConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }

    fn should_optimize(&self) -> bool {
        self.release
    }

    fn version(&self) -> Option<&str> {
        self.tools.sass.as_deref()
    }
}

impl IconConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }
}

impl TailwindCssConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }

    fn should_optimize(&self) -> bool {
        self.release
    }

    fn version(&self) -> Option<&str> {
        self.tools.tailwindcss.as_deref()
    }
}

impl CopyDirConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }
}

impl CopyFileConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }
}

impl RustAppConfig for RtcBuild {
    fn output_dir(&self) -> &Path {
        &self.staging_dist
    }

    fn public_url(&self) -> &str {
        &self.public_url
    }

    fn should_hash(&self) -> bool {
        self.filehash
    }

    fn should_optimize(&self) -> bool {
        self.release
    }

    fn wasm_bindgen_version(&self) -> Option<&str> {
        self.tools.wasm_bindgen.as_deref()
    }

    fn wasm_opt_version(&self) -> Option<&str> {
        self.tools.wasm_opt.as_deref()
    }

    fn format_preload(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        let pattern = self.pattern_preload.as_ref()?;

        let mut params: HashMap<String, String> = match self.pattern_params {
            Some(ref x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), self.public_url.clone());
        params.insert("js".to_owned(), script_path.to_owned());
        params.insert("wasm".to_owned(), wasm_path.to_owned());

        Some(pattern_evaluate(pattern, &params))
    }

    fn format_script(&self, script_path: &str, wasm_path: &str) -> Option<String> {
        let pattern = self.pattern_script.as_ref()?;

        let mut params: HashMap<String, String> = match self.pattern_params {
            Some(ref x) => x.clone(),
            None => HashMap::new(),
        };
        params.insert("base".to_owned(), self.public_url.clone());
        params.insert("js".to_owned(), script_path.to_owned());
        params.insert("wasm".to_owned(), wasm_path.to_owned());

        Some(pattern_evaluate(pattern, &params))
    }

    fn cargo_features(&self) -> Option<&trunk_util::Features> {
        Some(&self.cargo_features)
    }
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

const RELOAD_SCRIPT: &str = include_str!("../autoreload.js");

impl HtmlPipelineConfig for RtcBuild {
    fn append_body_str(&self) -> Option<Cow<'_, str>> {
        self.inject_autoloader
            .then(|| format!("<script>{}</script>", RELOAD_SCRIPT).into())
    }
}
