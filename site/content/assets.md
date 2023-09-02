+++
title = "Assets"
description = "Assets"
weight = 1
+++

Declaring assets to be processed by Trunk is simple and extensible.

Trunk is still a young project, and new asset types will be added as we move forward. Keep an eye on [trunk#3](https://github.com/thedodd/trunk/issues/3) for more information on planned asset types, implementation status, and please contribute to the discussion if you think something is missing.

# Link Asset Types
All link assets to be processed by Trunk must follow these three rules:
- Must be declared as a valid HTML `link` tag.
- Must have the attribute `data-trunk`.
- Must have the attribute `rel="{type}"`, where `{type}` is one of the asset types listed below.

This will typically look like: `<link data-trunk rel="{type}" href="{path}" ..other options here.. />`. Each asset type described below specifies the required and optional attributes for its asset type. All `<link data-trunk .../>` HTML elements will be replaced with the output HTML of the associated pipeline.

## rust
✅ `rel="rust"`: Trunk will compile the specified Cargo project as WASM and load it. This is optional. If not specified, Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `href`: (optional) the path to the `Cargo.toml` of the Rust project. If a directory is specified, then Trunk will look for the `Cargo.toml` in the given directory. If no value is specified, then Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `data-bin`: (optional) the name of the binary to compile and load. If the Cargo project has multiple binaries, this value will be required for proper functionality.
  - `data-type`: (optional) specifies how the binary should be loaded into the project. Can be set to `main` or `worker`. `main` is the default. There can only be one `main` link. For workers a wasm-bindgen javascript wrapper and the wasm file (with `_bg.wasm` suffix) is created, named after the binary name (if provided) or project name. See one of the webworker examples on how to load them.
  - `data-cargo-features`: (optional) Space or comma separated list of cargo features to activate.
  - `data-cargo-no-default-features`: (optional) Disables the default Cargo features.
  - `data-cargo-all-features`: (optional) Enables all Cargo features.
    - Neither compatible with `data-cargo-features` nor `data-cargo-no-default-features`.
  - `data-wasm-opt`: (optional) run wasm-opt with the set optimization level. The possible values are `0`, `1`, `2`, `3`, `4`, `s`, `z` or an _empty value_ for wasm-opt's default. Set this option to `0` to disable wasm-opt explicitly. The values `1-4` are increasingly stronger optimization levels for speed. `s` and `z` (z means more optimization) optimize for binary size instead. Only used in `--release` mode.
  - `data-keep-debug`: (optional) instruct `wasm-bindgen` to preserve debug info in the final WASM output, even for `--release` mode. This may conflict with the use of wasm-opt, so to be sure, it is recommended to set `data-wasm-opt="0"` when using this option.
  - `data-no-demangle`: (optional) instruct `wasm-bindgen` to not demangle Rust symbol names.
  - `data-reference-types`: (optional) instruct `wasm-bindgen` to enable [reference types](https://rustwasm.github.io/docs/wasm-bindgen/reference/reference-types.html).
  - `data-weak-refs`: (optional) instruct `wasm-bindgen` to enable [weak references](https://rustwasm.github.io/docs/wasm-bindgen/reference/weak-references.html).
  - `data-typescript`: (optional) instruct `wasm-bindgen` to output Typescript bindings. Defaults to false.
  - `data-loader-shim`: (optional) instruct `trunk` to create a loader shim for web workers. Defaults to false.

## sass/scss
✅ `rel="sass"` or `rel="scss"`: Trunk uses the official [dart-sass](https://github.com/sass/dart-sass) for compilation. Just link to your sass files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the sass/scss file to be processed.
- `data-inline`: (optional) this attribute will inline the compiled CSS from the SASS/SCSS file into a `<style>` tag instead of using a `<link rel="stylesheet">` tag.

## css
✅ `rel="css"`: Trunk will copy linked css files found in the source HTML without content modification. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the css file to be processed.
  - In the future, Trunk will resolve local `@imports`, will handle minification (see [trunk#7](https://github.com/thedodd/trunk/issues/7)), and we may even look into a pattern where any CSS found in the source tree will be bundled, which would enable a nice zero-config "component styles" pattern. See [trunk#3](https://github.com/thedodd/trunk/issues/3) for more details.

## tailwind
✅ `rel="tailwind-css"`: Trunk uses the official [tailwindcss cli](https://tailwindcss.com/blog/standalone-cli) for compilation. Just link to your tailwind css files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the sass/scss file to be processed.
- `data-inline`: (optional) this attribute will inline the compiled CSS from the tailwind compilation into a `<style>` tag instead of using a `<link rel="stylesheet">` tag.

## icon
✅ `rel="icon"`: Trunk will copy the icon image specified in the `href` attribute to the `dist` dir. This content is hashed for cache control.

## inline
✅ `rel="inline"`: Trunk will inline the content of the file specified in the `href` attribute into `index.html`. This content is copied exactly, no hashing is performed.
  - `type`: (optional) either `html`, `svg`, `css`, `js-module`, or `js`. If not present, the type is inferred by the file extension. `css` is wrapped in `style` tags, while
  `js` is wrapped in `script` tags.

## copy-file
✅ `rel="copy-file"`: Trunk will copy the file specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.

## copy-dir
✅ `rel="copy-dir"`: Trunk will recursively copy the directory specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.
  - `data-target-path`: (optional) Path where the directory is placed inside the dist dir. If not present the directory is placed in the dist root. The path must be a relative path without `..`.

# Script Asset Types
Script assets are bit more diverse.

## Script Assets
Classic script assets processed by Trunk must follow these three rules:
- Must be declared as a valid HTML `script` tag.
- Must have the attribute `data-trunk`.
- Must have the attribute `src`, pointing to a script file

This will typically look like: `<script data-trunk src="{path}" ..other options here.. />`. All `<script data-trunk .../>` HTML elements will be replaced with the output HTML of the associated pipeline.

Trunk will copy script files found in the source HTML without content modification. This content is hashed for cache control. The `src` attribute must be included in the script pointing to the script file to be processed.

## JS Snippets
JS snippets generated from the [wasm-bindgen JS snippets feature](https://rustwasm.github.io/docs/wasm-bindgen/reference/js-snippets.html) are automatically copied to the dist dir, hashed and ready to rock. No additional setup is required. Just use the feature in your application, and Trunk will take care of the rest.

# Images & Other Resources
Images and other resource types can be copied into the `dist` dir by adding a link like this to your source HTML: `<link data-trunk rel="copy-file" href="path/to/image"/>`. Any normal file type is supported. This will cause Trunk to find the target resource, and copy it to the `dist` dir unmodified. No hashing will be applied. The link itself will be removed from the HTML. To copy an entire directory of assets/images, you can use the following HTML: `<link data-trunk rel="copy-dir" href="path/to/images-dir"/>`.

This will allow your WASM application to reference images directly from the `dist` dir, and Trunk will ensure that the images are available in the `dist` dir to be served.

**NOTE:** as Trunk continues to mature, we will find better ways to include images and other resources. Hashing content for cache control is great, we just need to find a nice pattern to work with images referenced in Rust components. Please contribute to the discussion over in [trunk#9](https://github.com/thedodd/trunk/issues/9)! See you there.

# Directives
You can instruct Trunk to write the URL passed to `--public-url` to the HTML output by adding this to your `<head>`: `<base data-trunk-public-url/>`.

Trunk will set the `href` attribute of the element to the public URL. This changes the behavior of relative URLs to be relative to the public URL instead of the current location.

You can also access this value at runtime using `document.baseURI` which is useful for apps that need to know the base URL on which they're hosted (e.g. for routing).

# Hooks
If you find that you need Trunk to perform an additional build action that isn't supported directly, then Trunk's flexible hooks system can be used to launch external processes at various stages in the pipeline. Hooks can be declared exclusively in `Trunk.toml`, and consist of a `stage`, `command` and `command_arguments`:
  - `stage`: (required) one of `pre_build`, `build` or `post_build`. It specifies when in Trunk's build pipeline the hook is executed.
  - `command`: (required) the name or path to the desired executable.
  - `command_arguments`: (optional, defaults to none) any arguments to be passed, in the given order, to the executable.

At the relevant point for each stage, all hooks for that stage are spawned simultaneously. After this, Trunk immediately waits for all the hooks to exit before proceeding, except in the case of the `build` stage, described further below.

## Trunk's build process
This is a brief overview of Trunk's build process for the purpose of describing when hooks are executed. Please note that the exact ordering may change in the future to add new features.
  - Step 1 - Read and parse the HTML file.
  - Step 2 - Produce a plan of all assets to be built.
  - Step 3 - Build all assets in parallel.
  - Step 4 - Finalize and write assets to staging directory.
  - Step 5 - Write HTML to staging directory.
  - Step 6 - Replace `dist` directory contents with staging directory contents.

The hook stages correspond to this as follows:
  - `pre_build`: takes place before step 1.
  - `build`: takes place at the same time as step 3, executing in parallel with asset builds.
  - `post_build`: takes place after step 5 and before step 6.

## Hook Environment & Execution
All hooks are executed using the same `stdin` and `stdout` as trunk. The executable is expected to return an error code of `0` to indicate success. Any other code will be treated as an error and terminate the build process. Additionally, the following environment variables are provided to the process:
  - `TRUNK_PROFILE`: the build profile in use. Currently either `debug` or `release`.
  - `TRUNK_HTML_FILE`: the full path to the HTML file (typically `index.html` in `TRUNK_SOURCE_DIR`) used by trunk.
  - `TRUNK_SOURCE_DIR`: the full path to the source directory in use by Trunk. This is always the directory in which `TRUNK_HTML_FILE` resides.
  - `TRUNK_STAGING_DIR`: the full path of the Trunk staging directory.
  - `TRUNK_DIST_DIR`: the full path of the Trunk dist directory.
  - `TRUNK_PUBLIC_URL`: the configured public URL for Trunk.

# Auto-Reload
As of `v0.14.0`, Trunk now ships with the ability to automatically reload your web app as the Trunk build pipeline completes.
