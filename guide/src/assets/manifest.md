# Asset manifest

Trunk writes a versioned `trunk-assets.json` file into the distribution directory
on every successful build. It is generated from the same pipeline outputs used
to finalize the HTML; consumers do not need to inspect HTML or infer filenames.
Version 3 records a SHA-256 `content_digest` for every referenced output file
independently of whether subresource integrity (SRI) is enabled.

The manifest contains:

- The schema `version`, public URL prefix, and finalized HTML filename.
- CSS, JavaScript, and icon references in their source HTML order. Each entry
  includes its role, exact public URL, SHA-256 content digest, and emitted HTML
  attributes, including SRI when enabled.
- Rust/WASM bundle URLs and initialization configuration, including wasm-bindgen
  initializer behavior, compression, cross-origin mode, custom Trunk patterns,
  SHA-256 content digests for the JS/WASM/initializer files, and preload
  resources with their content digests, attributes, and SRI values.
- `document_assets`, the exact markup emitted by asset pipelines, in Trunk
  source-element order (with `null` order for generated elements without a
  source slot), together with its final `head` or `body` location. This
  includes inline CSS, scripts, HTML and SVG, generated WASM bootstrap markup,
  Trunk's loading attributes, and configured CSP nonce attributes. Consumers
  can use these fragments directly without parsing Trunk's generated HTML or
  recreating its asset tags.
- An empty `wasm` array for HTML-only builds.

Example:

```json
{
  "version": 3,
  "public_url": "/",
  "html": "index.html",
  "assets": [
    {
      "order": 1,
      "role": "stylesheet",
      "url": "/style-5ef2.css",
      "content_digest": "sha256-...",
      "attributes": {
        "integrity": "sha384-..."
      }
    }
  ],
  "wasm": [],
  "document_assets": [
    {
      "order": 1,
      "location": "head",
      "html": "<link rel=\"stylesheet\" href=\"/style-5ef2.css\" integrity=\"sha384-...\"/>"
    }
  ]
}
```

Consumers must reject unsupported manifest versions and may use `content_digest`
to detect stale or corrupted assets even when SRI is disabled. Versions 1 and 2
remain valid for the asset metadata they contain; they do not have
`document_assets`, so consumers requiring complete rendered asset markup should
require version 3.
`trunk-assets.json` is a reserved output filename; a build fails rather than
overwriting an asset at that path. Hooks can read the staging copy from
`TRUNK_ASSET_MANIFEST` during
`post_build`; after the build succeeds, the published copy is at the same
relative path under `TRUNK_DIST_DIR`.
