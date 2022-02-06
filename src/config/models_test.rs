use crate::config::models::*;

#[cfg(not(target_family = "windows"))]
#[test]
fn err_bad_trunk_toml_build_target() {
    let cwd = std::env::current_dir().expect("error getting cwd");
    let path = cwd.join("tests").join("data").join("bad-build-target.toml");
    let err = ConfigOpts::rtc_build(Default::default(), Some(path)).expect_err("expected config to err");
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
    let err = ConfigOpts::rtc_watch(Default::default(), Default::default(), Some(path)).expect_err("expected config to err");
    let expected_err = format!(
        r#"error taking canonical path to [watch].watch "fake-dir" in "{}/tests/data/bad-watch-path.toml""#,
        cwd.to_string_lossy(),
    );
    assert_eq!(err.to_string(), expected_err);
}
