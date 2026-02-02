use swc_common::{FileName, GLOBALS, Globals, Mark, SourceMap, sync::Lrc};
use swc_ecma_ast::EsVersion;
use swc_ecma_codegen::{Emitter, text_writer::JsWriter};
use swc_ecma_minifier::{
    optimize,
    option::{CompressOptions, ExtraOptions, MangleOptions, MinifyOptions},
};
use swc_ecma_parser::{EsSyntax, Syntax, parse_file_as_program};
use swc_ecma_transforms_base::{fixer::fixer, resolver};
use swc_ecma_visit::VisitMutWith;

/// Whether the JavaScript is a module or a global script
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsModuleType {
    /// Global script (non-module)
    Global,
    /// ES Module
    Module,
}

/// perform JS minification using swc
pub fn minify_js(bytes: Vec<u8>, mode: JsModuleType) -> Vec<u8> {
    let source = match String::from_utf8(bytes.clone()) {
        Ok(s) => s,
        Err(_) => return bytes,
    };

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(Lrc::new(FileName::Anon), source);

    let syntax = Syntax::Es(EsSyntax {
        jsx: false,
        ..Default::default()
    });
    let is_module = mode == JsModuleType::Module;

    let mut errors = vec![];
    let program = match parse_file_as_program(&fm, syntax, EsVersion::latest(), None, &mut errors) {
        Ok(p) => p,
        Err(err) => {
            tracing::warn!("Failed to parse JS for minification: {:?}", err);
            return bytes;
        }
    };

    if !errors.is_empty() {
        tracing::warn!("JS parsing had errors, skipping minification");
        return bytes;
    }

    // Each call creates a fresh Globals instance, ensuring isolation between concurrent runs.
    GLOBALS.set(&Globals::new(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        let mut program = program;
        program.visit_mut_with(&mut resolver(unresolved_mark, top_level_mark, is_module));

        let minify_options = MinifyOptions {
            compress: Some(CompressOptions {
                ..Default::default()
            }),
            mangle: Some(MangleOptions {
                ..Default::default()
            }),
            ..Default::default()
        };

        let extra = ExtraOptions {
            unresolved_mark,
            top_level_mark,
            mangle_name_cache: None,
        };

        let program = optimize(program, cm.clone(), None, None, &minify_options, &extra);

        let mut program = program;
        program.visit_mut_with(&mut fixer(None));

        let mut buf = vec![];
        {
            let mut emitter = Emitter {
                cfg: swc_ecma_codegen::Config::default().with_minify(true),
                cm: cm.clone(),
                comments: None,
                wr: JsWriter::new(cm.clone(), "\n", &mut buf, None),
            };

            if let Err(err) = emitter.emit_program(&program) {
                tracing::warn!("Failed to emit minified JS: {:?}", err);
                return bytes;
            }
        }

        buf
    })
}

/// perform CSS minification
pub fn minify_css(bytes: Vec<u8>) -> Vec<u8> {
    use lightningcss::stylesheet::*;

    /// wrap CSS minification to isolate borrowing the original content
    fn minify(css: &str) -> Result<String, ()> {
        // parse CSS

        let mut css = StyleSheet::parse(css, ParserOptions::default()).map_err(|err| {
            tracing::warn!("CSS parsing failed, skipping: {err}");
        })?;

        css.minify(MinifyOptions::default()).map_err(|err| {
            tracing::warn!("CSS minification failed, skipping: {err}");
        })?;

        Ok(css
            .to_css(PrinterOptions {
                minify: true,
                ..Default::default()
            })
            .map_err(|err| {
                tracing::warn!("CSS generation failed, skipping: {err}");
            })?
            .code)
    }

    match std::str::from_utf8(&bytes) {
        Ok(css) => minify(css).map(String::into_bytes).unwrap_or(bytes),
        Err(_) => bytes,
    }
}

/// perform HTML minification
pub fn minify_html(html: &[u8]) -> Vec<u8> {
    let mut minify_cfg = minify_html::Cfg::spec_compliant();
    minify_cfg.minify_css = true;
    minify_cfg.minify_js = true;
    minify_cfg.keep_closing_tags = true;
    minify_html::minify(html, &minify_cfg)
}
