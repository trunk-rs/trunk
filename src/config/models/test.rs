use crate::config::{
    models::*,
    rt::{BuildOptions, RtcBuild, RtcBuilder, RtcWatch, WatchOptions},
};
use semver::{Comparator, Op, Prerelease, Version, VersionReq};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[cfg(not(target_family = "windows"))]
#[tokio::test]
async fn err_bad_trunk_toml_build_target() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-build-target.toml");

    let (cfg, working_directory) = load(Some(path)).await.expect("expected config to parse");
    let err = RtcBuild::from_config(cfg, working_directory, |_, core| BuildOptions {
        core,
        inject_autoloader: false,
    })
    .await
    .expect_err("expected config to err");

    let expected_err = format!(
        r#"error getting the canonical path to the build target HTML file "{}/tests/data/index.html""#,
        cwd.to_string_lossy(),
    );
    assert_eq!(err.to_string(), expected_err);
}

#[cfg(not(target_family = "windows"))]
#[tokio::test]
async fn err_bad_trunk_toml_watch_path() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-watch-path.toml");
    let (cfg, working_directory) = load(Some(path)).await.expect("expected config to parse");
    let err = RtcWatch::from_config(cfg, working_directory, |_, core| WatchOptions {
        build: BuildOptions {
            core,
            inject_autoloader: false,
        },
        poll: None,
        enable_cooldown: false,
        clear_screen: false,
        no_error_reporting: false,
    })
    .await
    .expect_err("expected config to err");

    assert_eq!(
        err.to_string(),
        format!(
            r#"error taking the canonical path to the watch path: "{}/tests/data/fake-dir""#,
            cwd.display()
        )
    );
}

#[cfg(not(target_family = "windows"))]
#[tokio::test]
async fn err_bad_trunk_toml_watch_ignore() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-watch-ignore.toml");
    let (cfg, working_directory) = load(Some(path)).await.expect("expected config to parse");
    let err = RtcWatch::from_config(cfg, working_directory, |_, core| WatchOptions {
        build: BuildOptions {
            core,
            inject_autoloader: false,
        },
        poll: None,
        enable_cooldown: false,
        clear_screen: false,
        no_error_reporting: false,
    })
    .await
    .expect_err("expected config to err");
    assert_eq!(
        err.to_string(),
        format!(
            r#"error taking the canonical path to the watch ignore path: "{}/tests/data/fake.html""#,
            cwd.display()
        )
    );
}

async fn assert_trunk_version(
    path: impl AsRef<Path>,
    expected_version: VersionReq,
    pass: impl IntoIterator<Item = &'static str>,
    fail: impl IntoIterator<Item = &'static str>,
) {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join(path);

    let (cfg, working_directory) = load(Some(path)).await.expect("expected config to parse");
    let cfg = RtcBuild::from_config(cfg, working_directory, |_, core| BuildOptions {
        core,
        inject_autoloader: false,
    })
    .await
    .expect("configuration to build runtime");

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

#[tokio::test]
async fn trunk_version_none() {
    assert_trunk_version(
        "trunk-version-none.toml",
        VersionReq::STAR,
        ["0.10.0", "0.19.0-alpha.1", "1.0.0"],
        [],
    )
    .await;
}

#[tokio::test]
async fn trunk_version_any() {
    assert_trunk_version(
        "trunk-version-any.toml",
        VersionReq::STAR,
        ["0.10.0", "0.19.0-alpha.1", "1.0.0"],
        [],
    )
    .await
}

#[tokio::test]
async fn trunk_version_minor() {
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
    .await
}

#[tokio::test]
async fn trunk_version_range() {
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
    .await
}

#[tokio::test]
async fn trunk_version_prerelease() {
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
    .await
}

/// Ensure that we can load the example config
#[tokio::test]
async fn example_config() {
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
    let (_, _) = load(Some(target))
        .await
        .expect("example config should be parsable");
}
