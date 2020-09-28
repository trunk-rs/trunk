//! An insta snapshot test for the content generated from trunk on the app at examples/yew.
//!
//! See https://github.com/mitsuhiko/insta for info on how to update the snapshots when changes
//! are made to the target example app.

use std::fs::{read, read_dir, read_to_string};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Result};
use insta::assert_snapshot;

#[cfg(unix)]
const TRUNK_CMD: &str = "target/debug/trunk";
#[cfg(windows)]
const TRUNK_CMD: &str = "target\\debug\\trunk.exe";

#[test]
fn test_example_yew_trunk_build_output() -> Result<()> {
    if !PathBuf::from(TRUNK_CMD).exists() {
        bail!("ensure a debug build of trunk exists before running this test");
    }
    let cmd_path = PathBuf::from(TRUNK_CMD).canonicalize()?;
    let yew_example_dir = PathBuf::from("examples").join("yew");

    let clean_out = Command::new(&cmd_path).arg("clean").current_dir(&yew_example_dir).output()?;
    if !clean_out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&clean_out.stderr));
        eprintln!("{}", String::from_utf8_lossy(&clean_out.stdout));
        bail!("error while executing `trunk clean`");
    }
    let build_out = Command::new(&cmd_path).arg("build").current_dir(&yew_example_dir).output()?;
    if !build_out.status.success() {
        eprintln!("{}", String::from_utf8_lossy(&build_out.stderr));
        eprintln!("{}", String::from_utf8_lossy(&build_out.stdout));
        bail!("error while executing `trunk build`");
    }

    let mut artifacts = Artifacts::default();
    collect_artifacts(&yew_example_dir.join("dist"), &mut artifacts)?;
    match &artifacts.wasm {
        Some(wasm) => {
            let bytes = read(&wasm)?;
            let filename = wasm.to_string_lossy().to_string();
            let filehash = format!("{:x}", seahash::hash(&bytes));
            assert_snapshot!(filename, filehash);
        }
        None => bail!("wasm artifact not found for examples/yew"),
    };
    for txtfile in artifacts.text_files.iter() {
        let contents = read_to_string(&txtfile)?;
        let filename = txtfile.to_string_lossy().to_string().replace('/', "__");
        assert_snapshot!(filename, &contents);
    }
    Ok(())
}

#[derive(Default)]
struct Artifacts {
    text_files: Vec<PathBuf>,
    wasm: Option<PathBuf>,
}

fn collect_artifacts(dir: &Path, artifacts: &mut Artifacts) -> Result<()> {
    if dir.is_dir() {
        for entry in read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                collect_artifacts(&path, artifacts)?;
                continue;
            }
            let ext = path.extension();
            if ext.map(|ext| ext == "wasm").unwrap_or(false) {
                artifacts.wasm = Some(path);
            } else {
                artifacts.text_files.push(path);
            }
        }
    }
    Ok(())
}
