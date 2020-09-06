use std::path::PathBuf;
use serde::{Deserialize};

// Working around https://github.com/serde-rs/serde/issues/368
fn default_as_true() -> bool { true }
fn default_as_false() -> bool { true }
fn default_port() -> u16 { 8080 }
fn default_html_target() -> PathBuf { PathBuf::from("index.html") }
fn default_dist() -> PathBuf { PathBuf::from("dist") }
fn default_pub_url() -> String { "/".to_string() }

#[derive(Deserialize)]
pub struct Config {
    #[serde(default = "default_html_target")]
    pub html_target: PathBuf,
    #[serde(default = "default_as_true")]
    pub release: bool,
    #[serde(default = "default_dist")]
    pub dist: PathBuf,
    #[serde(default)]
    pub manifest: Option<PathBuf>,
    #[serde(default = "default_pub_url")]
    pub public_url: String,
    pub clean: CleanArgs,
    pub serve: ServeArgs,
    pub watch: WatchArgs,
}

#[derive(Deserialize)]
pub struct CleanArgs {
    #[serde(default = "default_as_false")]
    pub run_cargo_clean: bool
}

#[derive(Deserialize)]
pub struct ServeArgs {
    #[serde(default = "default_as_false")]
    pub open_browser: bool,
    #[serde(default)]
    pub ignored_paths: Option<Vec<PathBuf>>,
    #[serde(default = "default_port")]
    pub port: u16
}

#[derive(Deserialize)]
pub struct WatchArgs {
    #[serde(default)]
    pub ignored_paths: Option<Vec<PathBuf>>,
}
