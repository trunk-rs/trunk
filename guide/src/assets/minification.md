# Minification

Trunk supports minifying assets. This is disabled by default and can be controlled on various levels.

In any case, Trunk does not perform minification itself, but delegates the process to dependencies which do the actual
implementation. In cases where minification breaks things, it will, most likely, be an issue with that dependency.

Starting with Trunk 0.20.0, minification is disabled by default. It can be turned on from the command line using the
`--minify` (or `-M`) switch. Alternatively, it can be controlled using the `build.minify` field in the `Trunk.toml`
file. The value of this field is an enum, with the following possible values: `never` (default, never minify),
`on_release` (minify when running Trunk with `--release`), `always` (always minify).

When minification is enabled, all assets known to trunk (this excludes the `copy-dir` and `copy-file` opaque blobs to
Trunk), will get minified. It is possible to opt out of this process on a per-asset basis using the `data-no-minify`
attribute (see individual asset configuration). In this case, the asset will *never* get minified.

## Backends

The following table lists what is used to minify each kind of output, as implemented in Trunk's source:

| Output                                                                    | Backend                                                                                       |
|---------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------|
| HTML (`index.html`)                                                       | [`minify-html`](https://crates.io/crates/minify-html), also minifying inline CSS and JS in it |
| CSS (`rel="css"`)                                                         | [`lightningcss`](https://crates.io/crates/lightningcss)                                       |
| JavaScript (`<script data-trunk>`, and the `rel="rust"` loader and initializer) | `swc_ecma_minifier` (from [SWC](https://swc.rs))                                              |
| SASS/SCSS (`rel="sass"` / `rel="scss"`)                                   | The `sass` executable, with `--style compressed`                                              |
| Tailwind CSS (`rel="tailwind-css"`)                                       | The Tailwind CSS executable, with `--minify`                                                  |
| PNG icons (`rel="icon"`)                                                  | [`oxipng`](https://crates.io/crates/oxipng)                                                   |

If a JavaScript or CSS file cannot be parsed or minified, Trunk logs a warning and keeps the original content.
