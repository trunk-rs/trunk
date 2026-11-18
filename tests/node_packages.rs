//! Run `trunk build` on projects whose `[[node_packages]]` come from an npm compatible registry
//! served on 127.0.0.1, and check how links inside of package tarballs are handled: all symbolic
//! and hard links are skipped with a warning, as npm does, and nothing is ever written or read
//! outside of the install directory. Also check that tarballs are verified against the digests
//! announced by the registry before they are unpacked.
#![cfg(unix)]

use axum::{Json, Router, http::StatusCode, routing::get};
use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::{Compression, write::GzEncoder};
use rstest::rstest;
use std::{
    fs::{
        Permissions, create_dir, create_dir_all, metadata, read_dir, read_to_string,
        set_permissions, write,
    },
    os::unix::fs::{PermissionsExt, symlink as create_symlink},
    path::{Path, PathBuf},
    process::Output,
};

enum Entry {
    File(&'static str, &'static str),
    Directory(&'static str, u32),
    Symlink(&'static str, PathBuf),
    HardLink(&'static str, PathBuf),
}

/// Creates the tarball entries for a test case; the project gives access to paths outside of it.
type Entries = fn(&Project) -> Vec<Entry>;

fn file(path: &'static str, content: &'static str) -> Entry {
    Entry::File(path, content)
}

fn symlink(path: &'static str, target: impl Into<PathBuf>) -> Entry {
    Entry::Symlink(path, target.into())
}

fn hard_link(path: &'static str, target: impl Into<PathBuf>) -> Entry {
    Entry::HardLink(path, target.into())
}

/// Build a gzipped package tarball.
fn tarball(entries: &[Entry]) -> Vec<u8> {
    let mut builder = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for entry in entries {
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o644);
        header.set_size(0);
        let result = match entry {
            Entry::File(path, content) => {
                header.set_entry_type(tar::EntryType::Regular);
                header.set_size(content.len() as u64);
                builder.append_data(&mut header, path, content.as_bytes())
            }
            Entry::Directory(path, mode) => {
                header.set_entry_type(tar::EntryType::Directory);
                header.set_mode(*mode);
                builder.append_data(&mut header, path, std::io::empty())
            }
            Entry::Symlink(path, target) => {
                header.set_entry_type(tar::EntryType::Symlink);
                builder.append_link(&mut header, path, target)
            }
            Entry::HardLink(path, target) => {
                header.set_entry_type(tar::EntryType::Link);
                builder.append_link(&mut header, path, target)
            }
        };
        result.expect("entry can be added to the tarball");
    }
    builder
        .into_inner()
        .and_then(|gz| gz.finish())
        .expect("tarball can be built")
}

/// The `dist.integrity` and `dist.shasum` values the registry announces for a tarball.
type Digests = fn(&[u8]) -> (Option<String>, Option<String>);

/// The digests npmjs.org announces for current packages.
fn valid_digests(tarball: &[u8]) -> (Option<String>, Option<String>) {
    (Some(sha512_integrity(tarball)), Some(sha1_shasum(tarball)))
}

fn sha512_integrity(data: &[u8]) -> String {
    use sha2::Digest;
    format!("sha512-{}", STANDARD.encode(sha2::Sha512::digest(data)))
}

fn sha1_shasum(data: &[u8]) -> String {
    use sha1::Digest;
    sha1::Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Serve `tarball` as `pkg@1.0.0` and return the registry URL.
///
/// If `tarball` is `None`, requesting the tarball fails with "404 Not Found".
async fn serve_registry(tarball: Option<Vec<u8>>, digests: Digests) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener can be bound");
    let registry = format!(
        "http://{}/",
        listener.local_addr().expect("listener has an address")
    );
    let (integrity, shasum) = digests(tarball.as_deref().unwrap_or_default());
    let metadata = serde_json::json!({
        "name": "pkg",
        "version": "1.0.0",
        "dist": {
            "shasum": shasum,
            "tarball": format!("{registry}pkg-1.0.0.tgz"),
            "fileCount": 0,
            "integrity": integrity,
        },
    });
    let app = Router::new()
        .route("/pkg/1.0.0", get(move || async move { Json(metadata) }))
        .route(
            "/pkg-1.0.0.tgz",
            get(move || async move { tarball.ok_or(StatusCode::NOT_FOUND) }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await });
    registry
}

struct Project {
    // Removed on drop.
    _tmpdir: tempfile::TempDir,
    root: PathBuf,
    /// The directory `pkg@1.0.0` is installed into.
    install_dir: PathBuf,
    /// A directory next to the project, containing only [`Project::secret`].
    outside: PathBuf,
}

impl Project {
    fn new() -> Self {
        let tmpdir = tempfile::tempdir().expect("temporary directory can be created");
        let root = tmpdir.path().join("project");
        let outside = tmpdir.path().join("outside");
        create_dir(&root).expect("project directory can be created");
        create_dir(&outside).expect("outside directory can be created");
        set_permissions(&outside, Permissions::from_mode(0o755))
            .expect("permissions of outside directory can be set");
        write(outside.join("SECRET"), "secret").expect("secret can be written");
        write(
            root.join("index.html"),
            "<!DOCTYPE html><html><head></head><body></body></html>",
        )
        .expect("index.html can be written");
        Self {
            install_dir: root.join("target/node_modules/pkg/1.0.0"),
            _tmpdir: tmpdir,
            root,
            outside,
        }
    }

    /// A file outside of the project, which a package must not be able to read.
    fn secret(&self) -> PathBuf {
        self.outside.join("SECRET")
    }

    /// Run `trunk build` with `pkg@1.0.0` served from a registry on 127.0.0.1.
    async fn build(&self, entries: Entries) -> Output {
        self.build_with(Some(tarball(&entries(self))), valid_digests)
            .await
    }

    /// Like [`Project::build`], with control over the tarball and the announced digests.
    async fn build_with(&self, tarball: Option<Vec<u8>>, digests: Digests) -> Output {
        let registry = serve_registry(tarball, digests).await;
        write(
            self.root.join("Trunk.toml"),
            format!(
                r#"
[[node_packages]]
registry = "{registry}"
name = "pkg"
version = "1.0.0"
"#
            ),
        )
        .expect("Trunk.toml can be written");

        tokio::process::Command::new(env!("CARGO_BIN_EXE_trunk"))
            .arg("build")
            .current_dir(&self.root)
            .env("TRUNK_SKIP_VERSION_CHECK", "true")
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .output()
            .await
            .expect("trunk can be run")
    }

    /// Asserts that the outside directory is unchanged.
    fn assert_nothing_outside(&self) {
        let names = read_dir(&self.outside)
            .expect("outside directory can be read")
            .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
            .collect::<Result<Vec<_>, _>>()
            .expect("outside directory can be read");
        assert_eq!(
            names,
            ["SECRET"],
            "trunk build wrote outside of the project"
        );
        assert_eq!(
            read_to_string(self.secret()).expect("secret can be read"),
            "secret"
        );
    }

    /// Asserts that the permissions of the outside directory are unchanged.
    fn assert_outside_permissions(&self) {
        let mode = metadata(&self.outside)
            .expect("outside directory exists")
            .permissions()
            .mode()
            & 0o7777;
        assert_eq!(
            mode, 0o755,
            "trunk build changed permissions outside of the project, {mode:o}"
        );
    }
}

fn output(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[rstest]
#[case::plain_files(
    |_: &Project| vec![file("package/package.json", "{}"), file("package/dist/index.js", "index")],
    &[("package.json", "{}"), ("dist/index.js", "index")],
    &[],
)]
#[case::top_level_symlink_to_file(
    |p: &Project| vec![symlink("MARKER", p.outside.join("MARKER")), file("package/MARKER", "marker")],
    &[("MARKER", "marker")],
    &[],
)]
#[case::top_level_symlink_to_directory(
    |p: &Project| vec![symlink("assets", &p.outside), file("package/assets/MARKER", "marker")],
    &[("assets/MARKER", "marker")],
    &[],
)]
#[case::symlink_to_file_inside_package(
    |_: &Project| vec![file("package/dist/index.js", "index"), symlink("package/index.js", "dist/index.js")],
    &[("dist/index.js", "index")],
    &["index.js"],
)]
#[case::symlink_to_directory_inside_package(
    |_: &Project| vec![file("package/dist/esm/index.js", "index"), symlink("package/lib/esm", "../dist/esm")],
    &[("dist/esm/index.js", "index")],
    &["lib/esm"],
)]
#[case::absolute_symlink_outside_package(
    |p: &Project| vec![file("package/package.json", "{}"), symlink("package/leak", p.secret())],
    &[("package.json", "{}")],
    &["leak"],
)]
#[case::relative_symlink_outside_package(
    |_: &Project| vec![file("other", "not part of the package"), file("package/package.json", "{}"), symlink("package/leak", "../other")],
    &[("package.json", "{}")],
    &["leak"],
)]
#[case::symlink_chain_outside_package(
    |_: &Project| vec![
        file("other", "not part of the package"),
        file("package/package.json", "{}"),
        symlink("package/a", "."),
        symlink("package/leak", "a/../other"),
    ],
    &[("package.json", "{}")],
    &["a", "leak"],
)]
#[case::symlink_loop(
    |_: &Project| vec![file("package/package.json", "{}"), symlink("package/a", "b"), symlink("package/b", "a")],
    &[("package.json", "{}")],
    &["a", "b"],
)]
#[case::file_through_symlinked_directory(
    |p: &Project| vec![symlink("package/d", &p.outside), file("package/d/MARKER", "marker")],
    &[("d/MARKER", "marker")],
    &[],
)]
#[case::directory_entry_through_symlink(
    |p: &Project| vec![
        file("package/package.json", "{}"),
        symlink("package/d", &p.outside),
        Entry::Directory("package/d", 0o777),
    ],
    &[("package.json", "{}")],
    &[],
)]
#[case::hard_link_inside_package(
    |_: &Project| vec![file("package/dist/index.js", "index"), hard_link("package/index.js", "package/dist/index.js")],
    &[("dist/index.js", "index")],
    &["index.js"],
)]
#[case::hard_link_outside_package(
    |p: &Project| vec![file("package/package.json", "{}"), hard_link("package/linked", p.secret())],
    &[("package.json", "{}")],
    &["linked"],
)]
#[case::hard_link_to_symlink(
    |_: &Project| vec![
        file("x", "not part of the package"),
        file("package/x", "x"),
        symlink("package/a/l", "../x"),
        hard_link("package/l2", "package/a/l"),
    ],
    &[("x", "x")],
    &["a/l", "l2"],
)]
#[tokio::test]
async fn installed(
    #[case] entries: Entries,
    #[case] files: &[(&str, &str)],
    #[case] skipped: &[&str],
) {
    let project = Project::new();
    let output = project.build(entries).await;
    project.assert_nothing_outside();
    project.assert_outside_permissions();
    let output_text = self::output(&output);
    assert!(
        output.status.success(),
        "trunk build failed:\n{output_text}"
    );

    assert!(!project.install_dir.join("package").exists());
    for (path, content) in files {
        let actual = read_to_string(project.install_dir.join(path))
            .unwrap_or_else(|err| panic!("{path} can not be read: {err}"));
        assert_eq!(&actual, content, "content of {path}");
    }
    assert_no_links(&project.install_dir);
    for path in skipped {
        assert!(
            project.install_dir.join(path).symlink_metadata().is_err(),
            "link {path} was not skipped"
        );
        assert!(
            output_text.contains(&format!("skipping link package/{path}")),
            "expected a warning for {path}:\n{output_text}"
        );
    }
}

/// Asserts that there are no symbolic links below `dir`.
fn assert_no_links(dir: &Path) {
    for entry in read_dir(dir).expect("directory can be read") {
        let entry = entry.expect("directory entry can be read");
        let file_type = entry.file_type().expect("file type can be read");
        assert!(!file_type.is_symlink(), "{:?} is a link", entry.path());
        if file_type.is_dir() {
            assert_no_links(&entry.path());
        }
    }
}

#[rstest]
#[case::no_package_directory(|_: &Project| vec![file("other/package.json", "{}")])]
#[case::package_directory_is_a_symlink(|p: &Project| vec![symlink("package", &p.outside)])]
#[tokio::test]
async fn rejected(#[case] entries: Entries) {
    let project = Project::new();
    let output = project.build(entries).await;
    project.assert_nothing_outside();
    project.assert_outside_permissions();
    assert!(
        !output.status.success(),
        "trunk build succeeded:\n{}",
        self::output(&output)
    );
}

#[tokio::test]
async fn existing_install_directory_is_left_alone() {
    let project = Project::new();
    create_dir_all(&project.install_dir).expect("install directory can be created");
    create_symlink(
        project.outside.join("MARKER"),
        project.install_dir.join("package.json"),
    )
    .expect("symlink can be created");
    let output = project
        .build(|_: &Project| vec![file("package/package.json", "{}")])
        .await;
    assert!(
        output.status.success(),
        "trunk build failed:\n{}",
        self::output(&output)
    );
    project.assert_nothing_outside();
}

#[rstest]
#[case::only_integrity(|t: &[u8]| (Some(sha512_integrity(t)), None))]
#[case::only_shasum(|t: &[u8]| (None, Some(sha1_shasum(t))))]
#[case::empty_integrity_falls_back_to_shasum(|t: &[u8]| (Some(String::new()), Some(sha1_shasum(t))))]
#[tokio::test]
async fn verified(#[case] digests: Digests) {
    let project = Project::new();
    let output = project
        .build_with(
            Some(tarball(&[file("package/package.json", "{}")])),
            digests,
        )
        .await;
    assert!(
        output.status.success(),
        "trunk build failed:\n{}",
        self::output(&output)
    );
    assert_eq!(
        read_to_string(project.install_dir.join("package.json")).expect("package.json can be read"),
        "{}"
    );
}

#[rstest]
#[case::integrity_mismatch(
    |_: &[u8]| (Some(sha512_integrity(b"other")), Some(sha1_shasum(b"other"))),
    "integrity check failed",
)]
#[case::matching_shasum_does_not_override_integrity(
    |t: &[u8]| (Some(sha512_integrity(b"other")), Some(sha1_shasum(t))),
    "integrity check failed",
)]
#[case::shasum_mismatch(|_: &[u8]| (None, Some(sha1_shasum(b"other"))), "integrity check failed")]
#[case::no_digests(|_: &[u8]| (None, None), "neither an integrity value nor a shasum")]
#[case::empty_digests(
    |_: &[u8]| (Some(String::new()), Some(String::new())),
    "neither an integrity value nor a shasum",
)]
#[case::malformed_integrity(|_: &[u8]| (Some("sha512-AAAA".into()), None), "wrong length")]
#[tokio::test]
async fn unverified(#[case] digests: Digests, #[case] message: &str) {
    let project = Project::new();
    let output = project
        .build_with(
            Some(tarball(&[file("package/package.json", "{}")])),
            digests,
        )
        .await;
    let output_text = self::output(&output);
    assert!(
        !output.status.success(),
        "trunk build succeeded:\n{output_text}"
    );
    assert!(
        output_text.contains(message),
        "expected {message:?} in the output:\n{output_text}"
    );
    assert!(
        !project.install_dir.exists(),
        "an unverified package was unpacked"
    );
}

#[tokio::test]
async fn missing_tarball() {
    let project = Project::new();
    let output = project.build_with(None, valid_digests).await;
    let output_text = self::output(&output);
    assert!(
        !output.status.success(),
        "trunk build succeeded:\n{output_text}"
    );
    assert!(
        output_text.contains("404"),
        "expected the status code in the output:\n{output_text}"
    );
    assert!(!project.install_dir.exists());
}
