use anyhow::{Context, Result};
use seahash::SeaHasher;
use serde::{Deserialize, Serialize};
use std::{
    future::Future,
    hash::Hasher,
    io::Read,
    path::{Component, Path, PathBuf},
};
use tokio::{fs, io::AsyncReadExt};

#[derive(Debug, Deserialize, Serialize)]
struct CacheRecord {
    key: String,
    output_root: PathBuf,
    outputs: Vec<CachedOutput>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CachedOutput {
    path: String,
    hash: String,
    #[serde(default)]
    kind: CachedOutputKind,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
enum CachedOutputKind {
    #[default]
    File,
    Directory,
}

/// A cache input is either a file whose contents affect the result, or a value
/// (such as a command-line argument) that should be included in the cache key.
pub(crate) enum CacheInput<'a> {
    File(&'a Path),
    Value(&'a str),
}

/// A cache for deterministic output files and directories from a set of inputs.
pub(crate) struct Cache {
    cache_path: PathBuf,
}

impl Cache {
    /// Create a cache whose metadata is stored at `cache_path`.
    pub(crate) fn new(cache_path: impl Into<PathBuf>) -> Self {
        Self {
            cache_path: cache_path.into(),
        }
    }

    /// Return generated output paths from the cache, or generate and record them on a miss.
    pub(crate) async fn try_get<F, Fut>(
        &self,
        inputs: &[CacheInput<'_>],
        generate: F,
    ) -> Result<Vec<PathBuf>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Vec<PathBuf>>>,
    {
        let key = match cache_key(inputs).await {
            Ok(key) => Some(key),
            Err(error) => {
                tracing::warn!("unable to hash cache inputs: {error:#}");
                None
            }
        };

        if let Some(key) = &key {
            match self.is_valid(key).await {
                Ok(Some(outputs)) => {
                    tracing::debug!(cache = %self.cache_path.display(), "reusing cached outputs");
                    return Ok(outputs);
                }
                Ok(None) => {}
                Err(error) => tracing::warn!("unable to inspect cache: {error:#}"),
            }
        }

        let outputs = generate().await?;

        if let Some(key) = key {
            match self.store(key, &outputs).await {
                Ok(stored_outputs) => return Ok(stored_outputs),
                Err(error) => tracing::warn!("unable to store cache: {error:#}"),
            }
        }

        Ok(outputs)
    }

    pub async fn is_valid(&self, key: &str) -> Result<Option<Vec<PathBuf>>> {
        let Ok(contents) = fs::read(&self.cache_path).await else {
            return Ok(None);
        };
        let Ok(record) = serde_json::from_slice::<CacheRecord>(&contents) else {
            return Ok(None);
        };

        if record.key != key
            || record.outputs.is_empty()
            || !is_safe_absolute_path(&record.output_root)
        {
            return Ok(None);
        }

        let mut output_paths = Vec::with_capacity(record.outputs.len());
        for output in record.outputs {
            let relative_path = Path::new(&output.path);
            if !is_safe_relative_path(relative_path) {
                return Ok(None);
            }

            let path = record.output_root.join(relative_path);
            if !fs::try_exists(&path).await? || output_kind(&path).await? != output.kind {
                return Ok(None);
            }
            if hash_output(&path, &output.kind).await? != output.hash {
                return Ok(None);
            }
            output_paths.push(path);
        }

        Ok(Some(output_paths))
    }

    pub async fn store(&self, key: String, outputs: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let mut output_paths = Vec::with_capacity(outputs.len());
        for output in outputs {
            let path = fs::canonicalize(output)
                .await
                .with_context(|| format!("error resolving output '{}'", output.display()))?;
            output_paths.push(path);
        }
        output_paths.sort();
        output_paths.dedup();
        if output_paths.is_empty() {
            anyhow::bail!("generator returned no output files or directories");
        }

        fn common_parent(paths: &[PathBuf]) -> Result<PathBuf> {
            let Some(mut parent) = paths.first().and_then(|path| path.parent()) else {
                anyhow::bail!("output has no parent directory");
            };

            loop {
                if paths.iter().all(|path| path.starts_with(parent)) {
                    return Ok(parent.to_path_buf());
                }
                let Some(next_parent) = parent.parent() else {
                    anyhow::bail!("outputs do not share a common root");
                };
                parent = next_parent;
            }
        }

        let output_root = common_parent(&output_paths)?;
        let mut cached_outputs = Vec::with_capacity(output_paths.len());
        for path in &output_paths {
            let relative_path = path
                .strip_prefix(&output_root)
                .context("output escaped its common output root")?;
            if !is_safe_relative_path(relative_path) {
                anyhow::bail!("invalid output path '{}'", path.display());
            }
            let kind = output_kind(path).await?;
            let hash = hash_output(path, &kind).await?;
            cached_outputs.push(CachedOutput {
                path: relative_path.to_string_lossy().into_owned(),
                hash,
                kind,
            });
        }

        let record = CacheRecord {
            key,
            output_root,
            outputs: cached_outputs,
        };
        let contents = serde_json::to_vec(&record).context("error serializing cache")?;
        let Some(cache_dir) = self.cache_path.parent() else {
            anyhow::bail!("cache path has no parent directory");
        };
        fs::create_dir_all(cache_dir)
            .await
            .context("error creating cache directory")?;
        fs::write(&self.cache_path, contents)
            .await
            .context("error writing cache")?;

        Ok(output_paths)
    }
}

async fn cache_key(inputs: &[CacheInput<'_>]) -> Result<String> {
    let mut hasher = SeaHasher::new();
    for input in inputs {
        match input {
            CacheInput::File(path) => {
                hash_field(&mut hasher, b"file");
                hash_field(&mut hasher, hash_file(path).await?.as_bytes());
            }
            CacheInput::Value(value) => {
                hash_field(&mut hasher, b"value");
                hash_field(&mut hasher, value.as_bytes());
            }
        }
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn hash_field(hasher: &mut impl Hasher, value: &[u8]) {
    hasher.write(&(value.len() as u64).to_le_bytes());
    hasher.write(value);
}

async fn output_kind(path: &Path) -> Result<CachedOutputKind> {
    let metadata = fs::metadata(path)
        .await
        .with_context(|| format!("error inspecting cache output '{}'", path.display()))?;
    if metadata.is_file() {
        Ok(CachedOutputKind::File)
    } else if metadata.is_dir() {
        Ok(CachedOutputKind::Directory)
    } else {
        anyhow::bail!(
            "cache output '{}' is not a file or directory",
            path.display()
        )
    }
}

async fn hash_output(path: &Path, kind: &CachedOutputKind) -> Result<String> {
    match kind {
        CachedOutputKind::File => hash_file(path).await,
        CachedOutputKind::Directory => hash_directory(path).await,
    }
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

async fn hash_directory(path: &Path) -> Result<String> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || hash_directory_sync(&path))
        .await
        .context("error awaiting cache directory hashing")?
}

fn hash_directory_sync(path: &Path) -> Result<String> {
    let mut directories = vec![(path.to_path_buf(), PathBuf::new())];
    let mut entries = Vec::new();

    while let Some((directory, relative_directory)) = directories.pop() {
        let read_dir = std::fs::read_dir(&directory)
            .with_context(|| format!("error reading cache directory '{}'", directory.display()))?;
        for entry in read_dir {
            let entry = entry.with_context(|| {
                format!("error reading cache directory '{}'", directory.display())
            })?;
            let relative_path = relative_directory.join(entry.file_name());
            let entry_path = entry.path();
            let file_type = entry.file_type().with_context(|| {
                format!("error inspecting cache output '{}'", entry_path.display())
            })?;
            if file_type.is_dir() {
                entries.push((
                    relative_path.clone(),
                    entry_path.clone(),
                    CachedOutputKind::Directory,
                ));
                directories.push((entry_path, relative_path));
            } else if file_type.is_file() {
                entries.push((relative_path, entry_path, CachedOutputKind::File));
            } else {
                anyhow::bail!(
                    "cache directory contains unsupported output '{}'",
                    entry_path.display()
                );
            }
        }
    }

    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = SeaHasher::new();
    hash_field(&mut hasher, b"directory");
    for (relative_path, path, kind) in entries {
        hash_field(&mut hasher, relative_path.to_string_lossy().as_bytes());
        match kind {
            CachedOutputKind::File => {
                hash_field(&mut hasher, b"file");
                let mut file = std::fs::File::open(&path).with_context(|| {
                    format!("error opening '{}' for cache hashing", path.display())
                })?;
                let mut file_hasher = SeaHasher::new();
                let mut buffer = [0_u8; 64 * 1024];
                loop {
                    let bytes_read = file.read(&mut buffer).with_context(|| {
                        format!("error reading '{}' for cache hashing", path.display())
                    })?;
                    if bytes_read == 0 {
                        break;
                    }
                    file_hasher.write(&buffer[..bytes_read]);
                }
                hash_field(
                    &mut hasher,
                    format!("{:x}", file_hasher.finish()).as_bytes(),
                );
            }
            CachedOutputKind::Directory => hash_field(&mut hasher, b"directory"),
        }
    }

    Ok(format!("{:x}", hasher.finish()))
}

fn is_safe_relative_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn is_safe_absolute_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, Component::ParentDir | Component::CurDir))
}

#[cfg(test)]
mod tests {
    use super::{Cache, CacheInput, cache_key};
    use anyhow::Result;
    use std::path::Path;

    async fn write(path: impl AsRef<Path>, contents: &[u8]) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(path, contents).await?;
        Ok(())
    }

    #[tokio::test]
    async fn cache_key_changes_with_input_tool_or_arguments() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let wasm = tmpdir.path().join("input.wasm");
        let tool = tmpdir.path().join("wasm-bindgen");
        write(&wasm, b"wasm v1").await?;
        write(&tool, b"tool v1").await?;

        let inputs = [
            CacheInput::File(&wasm),
            CacheInput::File(&tool),
            CacheInput::Value("--target=web"),
            CacheInput::Value("--out-name=app"),
        ];
        let original = cache_key(&inputs).await?;
        assert_eq!(original, cache_key(&inputs).await?);
        let other_args = [
            CacheInput::File(&wasm),
            CacheInput::File(&tool),
            CacheInput::Value("--target=nodejs"),
            CacheInput::Value("--out-name=app"),
        ];
        assert_ne!(original, cache_key(&other_args).await?);

        write(&wasm, b"wasm v2").await?;
        assert_ne!(original, cache_key(&inputs).await?);

        write(&wasm, b"wasm v1").await?;
        write(&tool, b"tool v2").await?;
        assert_ne!(original, cache_key(&inputs).await?);
        Ok(())
    }

    #[tokio::test]
    async fn stored_outputs_are_reused_when_unchanged() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        let app_js = bindgen_out.join("app.js");
        let app_wasm = bindgen_out.join("app_bg.wasm");
        let snippets = bindgen_out.join("snippets");
        let types = bindgen_out.join("app.d.ts");
        write(&app_js, b"js").await?;
        write(&app_wasm, b"wasm").await?;
        write(snippets.join("pkg/inline.js"), b"snippet").await?;
        write(&types, b"types").await?;

        let cache = Cache::new(&cache_path);
        cache
            .store("key".to_owned(), &[app_js, app_wasm, snippets, types])
            .await?;

        assert_eq!(cache.is_valid("key").await?.as_ref().map(Vec::len), Some(4));
        assert!(cache.is_valid("other-key").await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cache_misses_if_an_output_is_missing_or_modified() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        let app_js = bindgen_out.join("app.js");
        let app_wasm = bindgen_out.join("app_bg.wasm");
        write(&app_js, b"js").await?;
        write(&app_wasm, b"wasm").await?;

        let cache = Cache::new(&cache_path);
        cache
            .store("key".to_owned(), &[app_js.clone(), app_wasm.clone()])
            .await?;

        write(&app_js, b"changed js").await?;
        assert!(cache.is_valid("key").await?.is_none());

        write(&app_js, b"js").await?;
        tokio::fs::remove_file(&app_wasm).await?;
        assert!(cache.is_valid("key").await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cache_misses_when_snippet_files_change() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        let app_js = bindgen_out.join("app.js");
        let app_wasm = bindgen_out.join("app_bg.wasm");
        let snippets = bindgen_out.join("snippets");
        let inline_snippet = snippets.join("pkg/inline.js");
        let new_snippet = snippets.join("pkg/new.js");
        write(&app_js, b"js").await?;
        write(&app_wasm, b"wasm").await?;
        write(&inline_snippet, b"snippet").await?;

        let cache = Cache::new(&cache_path);
        cache
            .store("key".to_owned(), &[app_js, app_wasm, snippets])
            .await?;
        assert!(cache.is_valid("key").await?.is_some());

        write(&new_snippet, b"new snippet").await?;
        assert!(cache.is_valid("key").await?.is_none());

        tokio::fs::remove_file(&new_snippet).await?;
        tokio::fs::remove_file(&inline_snippet).await?;
        assert!(cache.is_valid("key").await?.is_none());

        write(&inline_snippet, b"snippet").await?;
        assert!(cache.is_valid("key").await?.is_some());
        write(&inline_snippet, b"changed snippet").await?;
        assert!(cache.is_valid("key").await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cache_rejects_recorded_paths_outside_output_directory() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        let app_js = bindgen_out.join("app.js");
        let app_wasm = bindgen_out.join("app_bg.wasm");
        write(&app_js, b"js").await?;
        write(&app_wasm, b"wasm").await?;

        let cache = Cache::new(&cache_path);
        cache.store("key".to_owned(), &[app_js, app_wasm]).await?;
        let mut record: serde_json::Value =
            serde_json::from_slice(&tokio::fs::read(&cache_path).await?)?;
        record["outputs"]
            .as_array_mut()
            .expect("outputs should be an array")
            .push(serde_json::json!({"path": "../outside", "hash": "0", "kind": "file"}));
        tokio::fs::write(&cache_path, serde_json::to_vec(&record)?).await?;

        assert!(cache.is_valid("key").await?.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn cache_is_reusable_after_a_cache_miss() -> Result<()> {
        let tmpdir = tempfile::tempdir()?;
        let bindgen_out = tmpdir.path().join("bindgen");
        let cache_path = bindgen_out.join(".trunk/cache.json");
        let app_js = bindgen_out.join("app.js");
        let app_wasm = bindgen_out.join("app_bg.wasm");
        write(&app_js, b"js v1").await?;
        write(&app_wasm, b"wasm v1").await?;

        let cache = Cache::new(&cache_path);
        cache
            .store("key-v1".to_owned(), &[app_js.clone(), app_wasm.clone()])
            .await?;
        assert!(cache.is_valid("key-v1").await?.is_some());

        write(&app_js, b"js v2").await?;
        write(&app_wasm, b"wasm v2").await?;
        assert!(cache.is_valid("key-v1").await?.is_none());

        cache
            .store("key-v2".to_owned(), &[app_js, app_wasm])
            .await?;
        assert!(cache.is_valid("key-v2").await?.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn try_get_generates_on_miss_and_skips_generation_on_hit() -> Result<()> {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tmpdir = tempfile::tempdir()?;
        let output_dir = tmpdir.path().join("bindgen");
        let cache_path = output_dir.join(".trunk/cache.json");
        let cache = Cache::new(&cache_path);
        let input = tmpdir.path().join("input.wasm");
        write(&input, b"input").await?;
        let inputs = [CacheInput::File(&input)];
        let generated = AtomicUsize::new(0);
        let outputs = vec![output_dir.join("app.js"), output_dir.join("app_bg.wasm")];

        let first = cache
            .try_get(&inputs, || async {
                generated.fetch_add(1, Ordering::SeqCst);
                write(&outputs[0], b"js").await?;
                write(&outputs[1], b"wasm").await?;
                Ok(outputs.clone())
            })
            .await?;
        let second = cache
            .try_get(&inputs, || async {
                generated.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            })
            .await?;
        assert_eq!(first, second);
        assert_eq!(first.len(), 2);
        assert_eq!(generated.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
