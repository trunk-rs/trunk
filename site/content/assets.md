+++
title = "Assets"
description = "Assets"
weight = 1
+++

Declaring assets to be processed by Trunk is simple and extensible. All assets to be processed by Trunk must follow these three rules:
- Must be declared as a valid HTML `link` tag.
- Must have the attribute `data-trunk`.
- Must have the attribute `rel="{type}"`, where `{type}` is one of the asset types listed below.

This will typically look like: `<link data-trunk rel="{type}" href="{path}" ..other options here.. />`. Each asset type described below specifies the required and optional attributes for its asset type. All `<link data-trunk .../>` HTML elements will be replaced with the output HTML of the associated pipeline.

# Asset Types
## rust
✅ `rel="rust"`: Trunk will compile the specified Cargo project as the main WASM application. This is optional. If not specified, Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `href`: (optional) the path to the `Cargo.toml` of the Rust project. If a directory is specified, then Trunk will look for the `Cargo.toml` in the given directory. If no value is specified, then Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `data-bin`: (optional) the name of the binary to compile and use as the main WASM application. If the Cargo project has multiple binaries, this value will be required for proper functionality.
  - `data-wasm-opt`: (optional) run wasm-opt with the set optimization level. wasm-opt is **turned off by default** but that may change in the future. The possible values are `0`, `1`, `2`, `3`, `4`, `s`, `z` or an _empty value_ for wasm-opt's default. Set this option to `0` to disable wasm-opt explicitly. The values `1-4` are increasingly stronger optimization levels for speed. `s` and `z` (z means more optimization) optimize for binary size instead.
  - `data-keep-debug`: (optional) instruct `wasm-bindgen` to preserve debug info in the final WASM output, even for `--release` mode.
  - `data-no-demangle`: (optional) instruct `wasm-bindgen` to not demangle Rust symbol names.

## sass/scss
✅ `rel="sass"` or `rel="scss"`: Trunk ships with a [built-in sass/scss compiler](https://github.com/compass-rs/sass-rs). Just link to your sass files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the sass/scss file to be processed.

## css
✅ `rel="css"`: Trunk will copy linked css files found in the source HTML without content modification. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the css file to be processed.
  - In the future, Trunk will resolve local `@imports`, will handle minification (see [trunk#7](https://github.com/thedodd/trunk/issues/3)), and we may even look into a pattern where any CSS found in the source tree will be bundled, which would enable a nice zero-config "component styles" pattern. See [trunk#3](https://github.com/thedodd/trunk/issues/3) for more details.

## icon
✅ `rel="icon"`: Trunk will copy the icon image specified in the `href` attribute to the `dist` dir. This content is hashed for cache control.

## inline
✅ `rel="inline"`: Trunk will inline the content of the file specified in the `href` attribute into `index.html`. This content is copied exactly, no hashing is performed.
  - `type`: (optional) either `html`, `css`, or `js`. If not present, the type is inferred by the file extension. `css` is wrapped in `style` tags, while
  `js` is wrapped in `script` tags.

## copy-file
✅ `rel="copy-file"`: Trunk will copy the file specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.

## copy-dir
✅ `rel="copy-dir"`: Trunk will recursively copy the directory specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.

## rust-worker
⏳ `rel="rust-worker"`: (in-progress) Trunk will compile the specified Rust project as a WASM web worker. The following attributes are required:
  - `href`: (optional) the path to the `Cargo.toml` of the Rust project. If a directory is specified, then Trunk will look for the `Cargo.toml` in the given directory. If no value is specified, then Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `data-bin`: (optional) the name of the binary to compile and use as the web worker. If the Cargo project has multiple binaries, this value will be required for proper functionality.

Trunk is still a young project, and new asset types will be added as we move forward. Keep an eye on [trunk#3](https://github.com/thedodd/trunk/issues/3) for more information on planned asset types, implementation status, and please contribute to the discussion if you think something is missing.

# JS Snippets
JS snippets generated from the [wasm-bindgen JS snippets feature](https://rustwasm.github.io/docs/wasm-bindgen/reference/js-snippets.html) are automatically copied to the dist dir, hashed and ready to rock. No additional setup is required. Just use the feature in your application, and Trunk will take care of the rest.

# Images & Other Resources
Images and other resource types can be copied into the `dist` dir by adding a link like this to your source HTML: `<link data-trunk rel="copy-file" href="path/to/image"/>`. Any normal file type is supported. This will cause Trunk to find the target resource, and copy it to the `dist` dir unmodified. No hashing will be applied. The link itself will be removed from the HTML. To copy an entire directory of assets/images, you can use the following HTML: `<link data-trunk rel="copy-dir" href="path/to/images-dir"/>`.

This will allow your WASM application to reference images directly from the `dist` dir, and Trunk will ensure that the images are available in the `dist` dir to be served.

**NOTE:** as Trunk continues to mature, we will find better ways to include images and other resources. Hashing content for cache control is great, we just need to find a nice pattern to work with images referenced in Rust components. Please contribute to the discussion over in [trunk#9](https://github.com/thedodd/trunk/issues/9)! See you there.

# Directives
You can instruct Trunk to write the URL passed to `--public-url` to the HTML output by adding this to your `<head>`: `<base data-trunk-public-url/>`.

Trunk will set the `href` attribute of the element to the public URL. This changes the behavior of relative URLs to be relative to the public URL instead of the current location.

You can also access this value at runtime using `document.baseURI` which is useful for apps that need to know the base URL on which they're hosted (e.g. for routing).
