# Asset manifest

Trunk writes a versioned `trunk-assets.json` file into the distribution directory
on every successful build. It is generated from the same pipeline outputs used
to finalize the HTML; consumers do not need to inspect HTML or infer filenames.

The manifest contains:

- The schema `version`, public URL prefix, and finalized HTML filename.
- CSS, JavaScript, and icon references in their source HTML order. Each entry
  includes its role, exact public URL, and emitted HTML attributes, including
  SRI when enabled.
- Rust/WASM bundle URLs and initialization configuration, including wasm-bindgen
  initializer behavior, compression, cross-origin mode, custom Trunk patterns,
  and the preload resources with their attributes and SRI values.
- An empty `wasm` array for HTML-only builds.

Example:

```json
{
  "version": 1,
  "public_url": "/",
  "html": "index.html",
  "assets": [
    {
      "order": 1,
      "role": "stylesheet",
      "url": "/style-5ef2.css",
      "attributes": {
        "integrity": "sha384-..."
      }
    }
  ],
  "wasm": []
}
```

Consumers must reject unsupported manifest versions. `trunk-assets.json` is a
reserved output filename; a build fails rather than overwriting an asset at that
path. Hooks can read the staging copy from `TRUNK_ASSET_MANIFEST` during
`post_build`; after the build succeeds, the published copy is at the same
relative path under `TRUNK_DIST_DIR`.
