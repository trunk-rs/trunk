use crate::config::models::*;
use semver::{Comparator, Op, Prerelease, Version, VersionReq};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[cfg(not(target_family = "windows"))]
#[test]
fn err_bad_trunk_toml_build_target() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-build-target.toml");
    let err =
        ConfigOpts::rtc_build(Default::default(), Some(path)).expect_err("expected config to err");
    let expected_err = format!(
        r#"error taking canonical path to [build].target "index.html" in "{}/tests/data/bad-build-target.toml""#,
        cwd.to_string_lossy(),
    );
    assert_eq!(err.to_string(), expected_err);
}

#[cfg(not(target_family = "windows"))]
#[test]
fn err_bad_trunk_toml_watch_path() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-watch-path.toml");
    let err = ConfigOpts::rtc_watch(Default::default(), Default::default(), Some(path))
        .expect_err("expected config to err");
    let expected_err = format!(
        r#"error taking canonical path to [watch].watch "fake-dir" in "{}/tests/data/bad-watch-path.toml""#,
        cwd.to_string_lossy(),
    );
    assert_eq!(err.to_string(), expected_err);
}

#[cfg(not(target_family = "windows"))]
#[test]
fn err_bad_trunk_toml_watch_ignore() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-watch-ignore.toml");
    let err = ConfigOpts::rtc_watch(Default::default(), Default::default(), Some(path))
        .expect_err("expected config to err");
    let expected_err = format!(
        r#"error taking canonical path to [watch].ignore "fake.html" in "{}/tests/data/bad-watch-ignore.toml""#,
        cwd.to_string_lossy(),
    );
    assert_eq!(err.to_string(), expected_err);
}

fn assert_trunk_version(
    path: impl AsRef<Path>,
    expected_version: VersionReq,
    pass: impl IntoIterator<Item = &'static str>,
    fail: impl IntoIterator<Item = &'static str>,
) {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join(path);

    let cfg =
        ConfigOpts::rtc_build(Default::default(), Some(path)).expect("expected config to parse");

    assert_eq!(cfg.core.trunk_version, expected_version);

    for version in pass {
        assert!(
            crate::version::enforce_version_with(
                &cfg.core.trunk_version,
                Version::parse(version).expect("version must parse")
            )
            .is_ok(),
            "Version should pass: {version}"
        );
    }

    for version in fail {
        assert!(
            crate::version::enforce_version_with(
                &cfg.core.trunk_version,
                Version::parse(version).expect("version must parse")
            )
            .is_err(),
            "Version should fail: {version}"
        );
    }
}

#[test]
fn trunk_version_none() {
    assert_trunk_version(
        "trunk-version-none.toml",
        VersionReq::STAR,
        ["0.10.0", "0.19.0-alpha.1", "1.0.0"],
        [],
    );
}

#[test]
fn trunk_version_any() {
    assert_trunk_version(
        "trunk-version-any.toml",
        VersionReq::STAR,
        ["0.10.0", "0.19.0-alpha.1", "1.0.0"],
        [],
    )
}

#[test]
fn trunk_version_minor() {
    assert_trunk_version(
        "trunk-version-minor.toml",
        VersionReq {
            comparators: vec![Comparator {
                op: Op::Caret,
                major: 0,
                minor: Some(19),
                patch: None,
                pre: Prerelease::EMPTY,
            }],
        },
        ["0.19.0", "0.19.1"],
        ["0.18.1", "0.19.0-alpha.1", "0.20.0"],
    )
}

#[test]
fn trunk_version_range() {
    assert_trunk_version(
        "trunk-version-range.toml",
        VersionReq {
            comparators: vec![
                Comparator {
                    op: Op::GreaterEq,
                    major: 0,
                    minor: Some(17),
                    patch: None,
                    pre: Prerelease::EMPTY,
                },
                Comparator {
                    op: Op::Less,
                    major: 0,
                    minor: Some(19),
                    patch: None,
                    pre: Prerelease::EMPTY,
                },
            ],
        },
        ["0.17.0", "0.17.1", "0.18.0", "0.18.1"],
        ["0.19.0", "0.17.0-alpha.1"],
    )
}

#[test]
fn trunk_version_prerelease() {
    assert_trunk_version(
        "trunk-version-prerelease.toml",
        VersionReq {
            comparators: vec![Comparator {
                op: Op::Caret,
                major: 0,
                minor: Some(19),
                patch: Some(0),
                pre: Prerelease::new("alpha.1").expect("prerelease must parse"),
            }],
        },
        [
            "0.19.0-alpha.1",
            "0.19.0-alpha.2",
            "0.19.0-beta.1",
            "0.19.0",
        ],
        ["0.18.0", "0.18.0-alpha.1"],
    )
}

/// Ensure that we can load the example config
#[test]
fn example_config() {
    let dir = tempdir().expect("should be able to create temp directory");

    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("Trunk.toml");
    let target = dir.path().join("Trunk.toml");

    // copy to temp dir
    fs::copy(path, &target).expect("should copy file");
    // create a dummy index.html
    fs::write(dir.path().join("index.html"), r#""#)
        .expect("should be able to write temporary file");

    // check
    ConfigOpts::file_and_env_layers(Some(target)).expect("example config should be parsable");
}
