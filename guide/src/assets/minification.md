# Minification

Trunk supports minifying assets. This is disabled by default and can be controlled on various levels.

In any case, Trunk does not perform minification itself, but delegates the process to dependencies which do the actual
implementation. In cases where minification breaks things, it will, most likely, be an issue with that dependency.

The current backends are:

- HTML: [`minify-html`](https://github.com/wilsonzlin/minify-html), including inline CSS and JavaScript in HTML.
- CSS assets: [`lightningcss`](https://github.com/parcel-bundler/lightningcss).
- JavaScript assets, ES modules, and generated Rust/WASM loader snippets: [`swc`](https://swc.rs/).
- SASS/SCSS assets: the configured `sass`/`dart-sass` tool, using compressed output when minification is enabled.
- Tailwind CSS assets: the configured `tailwindcss` tool, using its `--minify` flag.
- PNG icons: [`oxipng`](https://github.com/shssoichiro/oxipng); other icon/image formats are copied unchanged.

If a backend cannot parse or minify an individual CSS or JavaScript asset, Trunk logs a warning and keeps the original
asset bytes for that file.

Starting with Trunk 0.20.0, minification is disabled by default. It can be turned on from the command line using the
`--minify` (or `-M`) switch. Alternatively, it can be controlled using the `build.minify` field in the `Trunk.toml`
file. The value of this field is an enum, with the following possible values: `never` (default, never minify),
`on_release` (minify when running Trunk with `--release`), `always` (always minify).

When minification is enabled, all assets known to trunk (this excludes the `copy-dir` and `copy-file` opaque blobs to
Trunk), will get minified. It is possible to opt out of this process on a per-asset basis using the `data-no-minify`
attribute (see individual asset configuration). In this case, the asset will *never* get minified.
