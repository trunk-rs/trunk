mod models;
pub use models::*;
use std::path::PathBuf;


pub fn read_config(path: Option<PathBuf>) -> Config {
    let path = path.unwrap_or(PathBuf::from("Trunk.toml"));
    let toml_string = std::fs::read_to_string(path).expect("Reading TOML config file failed");

    toml::from_str(&toml_string).expect("Parsing TOML config file failed")
}

