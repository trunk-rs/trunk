//! Asset Pipelines.

mod copy_dir;
mod copy_file;
mod css;
mod icon;
mod inline;
mod js;
mod output;
mod pipeline;
mod rust;
mod sass;
mod tailwind_css;

pub use copy_dir::{CopyDir, CopyDirConfig, CopyDirOutput};
pub use copy_file::{CopyFile, CopyFileConfig, CopyFileOutput};
pub use css::{Css, CssConfig, CssOutput};
pub use icon::{Icon, IconConfig, IconOutput};
pub use inline::{Inline, InlineOutput};
pub use js::{Js, JsConfig, JsOutput};
pub use output::Output;
pub use pipeline::Pipeline;
pub use rust::{RustApp, RustAppConfig, RustAppOutput};
pub use sass::{Sass, SassConfig, SassOutput};
pub use tailwind_css::{TailwindCss, TailwindCssConfig, TailwindCssOutput};
