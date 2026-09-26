use anyhow::{Context, Result};
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    hash::Hasher,
    path::{Component, Path, PathBuf},
};
use tokio::{fs, io::AsyncReadExt};

use super::SNIPPETS_DIR;

#[derive(Debug, Deserialize, Serialize)]
struct CacheRecord {
    key: String,
    outputs: Vec<CachedOutput>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CachedOutput {
    path: String,
    hash: String,
}

/// Build a cache key from the input bytes, selected tool, and full argument list.
pub(crate) async fn cache_key(
    wasm_path: &Path,
    wasm_bindgen: &Path,
    args: &[&str],
) -> Result<String> {
    let mut hasher = SeaHasher::new();
    hash_field(&mut hasher, hash_file(wasm_path).await?.as_bytes());
    hash_field(&mut hasher, hash_file(wasm_bindgen).await?.as_bytes());
    for arg in args {
        hash_field(&mut hasher, arg.as_bytes());
    }

    Ok(format!("{:x}", hasher.finish()))
}

/// Return true only when the cache key matches and all recorded outputs remain intact.
pub(crate) async fn is_valid(
    cache_path: &Path,
    bindgen_out: &Path,
    key: &str,
    required_outputs: &[PathBuf],
) -> Result<bool> {
    let Ok(contents) = fs::read(cache_path).await else {
        return Ok(false);
    };
    let Ok(record) = serde_json::from_slice::<CacheRecord>(&contents) else {
        return Ok(false);
    };

    if record.key != key {
        return Ok(false);
    }

    let recorded_paths = record
        .outputs
        .iter()
        .map(|output| output.path.clone())
        .collect::<HashSet<_>>();
    if required_outputs
        .iter()
        .any(|path| !recorded_paths.contains(&path.to_string_lossy().into_owned()))
    {
        return Ok(false);
    }

    // Snippets are copied as a directory. Even a newly added file must cause a
    // miss, otherwise the build would publish files outside the cached snapshot.
    let snippets_dir = bindgen_out.join(SNIPPETS_DIR);
    let mut snippets = Vec::new();
    if fs::try_exists(&snippets_dir).await? {
        collect_files(&snippets_dir, &mut snippets).await?;
    }
    let recorded_snippets = record
        .outputs
        .iter()
        .filter(|output| Path::new(&output.path).starts_with(SNIPPETS_DIR))
        .map(|output| bindgen_out.join(&output.path))
        .collect::<HashSet<_>>();
    let current_snippets = snippets.into_iter().collect::<HashSet<_>>();
    if current_snippets != recorded_snippets {
        return Ok(false);
    }

    for output in record.outputs {
        let relative_path = Path::new(&output.path);

        if relative_path.as_os_str().is_empty() {
            return Ok(false);
        }

        if !relative_path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        {
            return Ok(false);
        }

        let path = bindgen_out.join(relative_path);
        if !fs::try_exists(&path).await? {
            return Ok(false);
        }

        if hash_file(&path).await? != output.hash {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Record the files wasm-bindgen produced after a successful invocation.
pub(crate) async fn store(
    cache_path: &Path,
    bindgen_out: &Path,
    key: String,
    required_outputs: &[PathBuf],
) -> Result<()> {
    let mut output_paths = Vec::new();
    collect_files(bindgen_out, &mut output_paths).await?;
    if let Some(cache_dir) = cache_path.parent() {
        output_paths.retain(|path| !path.starts_with(cache_dir));
    }
    output_paths.sort();
    let mut outputs = Vec::with_capacity(output_paths.len());
    for path in output_paths {
        let relative_path = path
            .strip_prefix(bindgen_out)
            .context("wasm-bindgen output escaped its output directory")?;
        outputs.push(CachedOutput {
            path: relative_path.to_string_lossy().into_owned(),
            hash: hash_file(&path).await?,
        });
    }

    let recorded_paths = outputs
        .iter()
        .map(|output| output.path.as_str())
        .collect::<HashSet<_>>();
    for required in required_outputs {
        if !recorded_paths.contains(required.to_string_lossy().as_ref()) {
            anyhow::bail!(
                "required wasm-bindgen output '{}' was not generated",
                required.display()
            );
        }
    }

    let record = CacheRecord { key, outputs };
    let contents = serde_json::to_vec(&record).context("error serializing wasm-bindgen cache")?;
    let Some(cache_dir) = cache_path.parent() else {
        anyhow::bail!("wasm-bindgen cache path has no parent directory");
    };
    fs::create_dir_all(cache_dir)
        .await
        .context("error creating wasm-bindgen cache directory")?;
    fs::write(cache_path, contents)
        .await
        .context("error writing wasm-bindgen cache")?;

    Ok(())
}

fn hash_field(hasher: &mut impl Hasher, value: &[u8]) {
    hasher.write(&(value.len() as u64).to_le_bytes());
    hasher.write(value);
}

async fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)
        .await
        .with_context(|| format!("error opening '{}' for cache hashing", path.display()))?;
    let mut hasher = SeaHasher::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .await
            .with_context(|| format!("error reading '{}' for cache hashing", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.write(&buffer[..bytes_read]);
    }

    Ok(format!("{:x}", hasher.finish()))
}

async fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .await
        .with_context(|| format!("error reading '{}' for cache", directory.display()))?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_type = entry.file_type().await?;
        if file_type.is_file() {
            files.push(path);
        } else if file_type.is_dir() {
            Box::pin(collect_files(&path, files)).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cache_key, is_valid, store};
    use anyhow::Result;
    use std::path::{Path, PathBuf};

    const REQUIRED_OUTPUTS: &[&str] = &["app.js", "app_bg.wasm"];

    async fn write(path: impl AsRef<Path>, contents: &[u8]) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, contents).await?;
        Ok(())
    }

    fn required_outputs() -> Vec<PathBuf> {
        REQUIRED_OUTPUTS.iter().map(PathBuf::from).collect()
    }

    #[tokio::test]
    async fn cache_key_changes_with_input_tool_or_arguments() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let wasm = tmpdir.path().join("input.wasm");
        let tool = tmpdir.path().join("wasm-bindgen");
        write(&wasm, b"wasm v1").await?;
        write(&tool, b"tool v1").await?;

        let original = cache_key(&wasm, &tool, &["--target=web", "--out-name=app"]).await?;
        assert_eq!(
            original,
            cache_key(&wasm, &tool, &["--target=web", "--out-name=app"]).await?
        );
        assert_ne!(
            original,
            cache_key(&wasm, &tool, &["--target=nodejs", "--out-name=app"]).await?
        );

        write(&wasm, b"wasm v2").await?;
        assert_ne!(
            original,
            cache_key(&wasm, &tool, &["--target=web", "--out-name=app"]).await?
        );

        write(&wasm, b"wasm v1").await?;
        write(&tool, b"tool v2").await?;
        assert_ne!(
            original,
            cache_key(&wasm, &tool, &["--target=web", "--out-name=app"]).await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn stored_outputs_are_reused_when_unchanged() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        write(bindgen_out.join("app.js"), b"js").await?;
        write(bindgen_out.join("app_bg.wasm"), b"wasm").await?;
        write(bindgen_out.join("snippets/pkg/inline.js"), b"snippet").await?;
        write(bindgen_out.join("app.d.ts"), b"types").await?;

        store(
            &cache_path,
            &bindgen_out,
            "key".to_owned(),
            &[
                PathBuf::from("app.js"),
                PathBuf::from("app_bg.wasm"),
                PathBuf::from("app.d.ts"),
            ],
        )
        .await?;

        assert!(is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        assert!(!is_valid(&cache_path, &bindgen_out, "other-key", &required_outputs()).await?);
        assert!(
            !is_valid(
                &cache_path,
                &bindgen_out,
                "key",
                &[PathBuf::from("missing.js")]
            )
            .await?
        );
        Ok(())
    }

    #[tokio::test]
    async fn cache_misses_if_an_output_is_missing_or_modified() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        write(bindgen_out.join("app.js"), b"js").await?;
        write(bindgen_out.join("app_bg.wasm"), b"wasm").await?;
        store(
            &cache_path,
            &bindgen_out,
            "key".to_owned(),
            &required_outputs(),
        )
        .await?;

        write(bindgen_out.join("app.js"), b"changed js").await?;
        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);

        write(bindgen_out.join("app.js"), b"js").await?;
        tokio::fs::remove_file(bindgen_out.join("app_bg.wasm")).await?;
        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        Ok(())
    }

    #[tokio::test]
    async fn cache_misses_when_snippet_files_change() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        write(bindgen_out.join("app.js"), b"js").await?;
        write(bindgen_out.join("app_bg.wasm"), b"wasm").await?;
        write(bindgen_out.join("snippets/pkg/inline.js"), b"snippet").await?;
        store(
            &cache_path,
            &bindgen_out,
            "key".to_owned(),
            &required_outputs(),
        )
        .await?;

        assert!(is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        write(bindgen_out.join("snippets/pkg/new.js"), b"new snippet").await?;
        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);

        tokio::fs::remove_file(bindgen_out.join("snippets/pkg/new.js")).await?;
        tokio::fs::remove_file(bindgen_out.join("snippets/pkg/inline.js")).await?;
        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);

        write(bindgen_out.join("snippets/pkg/inline.js"), b"snippet").await?;
        assert!(is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        write(
            bindgen_out.join("snippets/pkg/inline.js"),
            b"changed snippet",
        )
        .await?;
        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        Ok(())
    }

    #[tokio::test]
    async fn cache_rejects_recorded_paths_outside_output_directory() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        write(bindgen_out.join("app.js"), b"js").await?;
        write(bindgen_out.join("app_bg.wasm"), b"wasm").await?;
        store(
            &cache_path,
            &bindgen_out,
            "key".to_owned(),
            &required_outputs(),
        )
        .await?;
        let mut record: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&cache_path).await?)?;
        record["outputs"]
            .as_array_mut()
            .expect("outputs should be an array")
            .push(serde_json::json!({"path": "../outside", "hash": "0"}));
        tokio::fs::write(&cache_path, serde_json::to_vec(&record)?).await?;

        assert!(!is_valid(&cache_path, &bindgen_out, "key", &required_outputs()).await?);
        Ok(())
    }
}
