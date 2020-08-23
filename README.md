<h1 align="center">trunk</h1>
<div align="center">
  <strong>
    Build, bundle & ship your Rust WASM application to the web.
  </strong>
  <br/>
  <i>
    ”Pack your things, we’re going on an adventure!” ~ Ferris
  </i>
</div>
<br/>

Goals:
- Leverage the wasm-bindgen ecosystem for all things related to building Rust WASM for the web.
- Simple, zero-config bundling of WASM, JS loader & other assets (images, css, scss) via a source HTML file.
- Rust WASM first. Not the other way around.

Major inspiration from [ParcelJS](https://parceljs.org).

## commands
### build
- ✅ uses a source `index.html` file to drive asset bundling, compilation, transpilation &c.
  - ✅ all bundled resources are hashed for cache control.
- ✅ invoke `cargo build`.
  - ✅ can specify `--release` mode.
  - ✅ uses the `Cargo.toml` in the current directory.
  - future: can specify a different `Cargo.toml` manifest.
- ✅ invoke `wasm-bindgen` to produce the final WASM & the JS loader.
  - ✅ hash original WASM from `cargo build` for cache busting on the web. Applies to JS loader file as well.
- ✅ SASS/SCSS compilation, hashing and inclusion in final output.
  - ✅ currently, this is a required option (because it is what I need for my project).
  - ✅ this will be based purely on a sass/scss file referenced in the source `index.html`.
  - future: we can look into a convention where we descend over the source tree and find any sass/scss/css, and add them all to a single output file which will be injected into the output HTML. The "component pattern".
- ✅ generates an `index.html` file based on the source HTML file.
  - ✅ adds the code needed for loading and running the WASM app, all referencing the hashed files for cache busting.
  - ✅ this will seamlessly include all of the content of the source HTML.
- ✅ puts all bundles assets into `dist` folder, including generated `index.html`, the application WASM, the JS loader for the WASM, the SASS/SCSS output CSS, and any other assets which were compiled/bundled in the process.
  - ✅ output dir is configurable.
  - ✅ all intermediate artifacts are stored under the `target` dir, as is normal Rust/Cargo practice. Helps to reduce clutter.

### watch
- same as build, except will use https://github.com/notify-rs/notify to trigger new build invocations.

### serve
- ✅ serve content on `:8080`.
  - ✅ port is configurable.
  - ✅ directory to serve content from is configurable.
  - ✅ serves the `index.html` as the content of `/` (which is what you would expect).
- invoke the `watch` functionality in a background thread.
  - use SSE to communicate with browser app to trigger reloads.
  - use websockets to do WASM HMR (this will probably be a beast, may not even be browser supported quite yet, we'll see).

### ship
- should do everything that build does, except in release mode, perhaps with some size optimizations and such.

### clean
- should just clean out the `dist` dir, and any other artifacts.
- optionally do a `cargo clean` as well (-c/--cargo or the like).
