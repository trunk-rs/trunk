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

Inspiration primarily from [ParcelJS](https://parceljs.org).

## getting started
```bash
# Install trunk.
cargo install https://github.com/thedodd/trunk
# After our first release: `cargo install trunk`

# Install wasm-bindgen-cli.
cargo install wasm-bindgen-cli
# NOTE: in the future, trunk will do this for you.
```

Get setup with your favorite wasm-bindgen based framework. [Yew](https://github.com/yewstack/yew) & [Seed](https://github.com/seed-rs/seed) are the most popular options today, but there are others. `trunk` will work with any `wasm-bindgen` based framework, just remember that your application's entry point must be setup as follows:

```rust
// The name of this function doesn't matter, but the attribute does.
#[wasm_bindgen(start)]
pub fn run() {
    // ... your app setup code here ...
}
```

Trunk uses a source HTML file to drive all asset building and bundling. Trunk also ships with a built-in SASS/SCSS compiler, so let's get started with the following example. Copy this HTML to your cargo project's `src` dir at `src/index.html`:

```html
<html>
  <head>
    <link rel="stylesheet" href="index.scss"/>
  </head>
</html>
```

`trunk build src/index.html` will produce the following HTML at `dist/index.html`, along with the compiled SCSS, WASM & the JS loader for the WASM:

```html
<html>
  <head>
    <link rel="stylesheet" href="/index.700471f89cef12e4.css">
    <script type="module">
      import init from '/index-719b4e04e016028b.js';
      init('/index-719b4e04e016028b_bg.wasm');
    </script>
  </head>
</html>
```

The contents of your `dist` dir are now ready to be served on the web. But that's not all! Trunk has even more useful features. Continue reading to learn about other Trunk commands and supported asset types.

## commands
### build
`trunk build <src/index.html>` runs a cargo build targeting the wasm32 instruction set, runs `wasm-bindgen` on the built WASM, spawns asset build pipelines for any assets defined in the target `index.html`.

Trunk leverages Rust's powerful concurrency primitives for maximum build speeds.

### watch
`trunk watch <src/index.html>` does the same thing as `trunk build`, but watches the filesystem for changes, triggering new builds as changes are detected.

### serve
`trunk serve <src/index.html>` does the same thing as `trunk watch`, but also spawns a web server.

### clean
`trunk clean` cleans up any build artifacts generated from earlier builds.

## assets
Trunk is still a young project, and new asset types will be added as we move forward. Keep an eye on [trunk#3](https://github.com/thedodd/trunk/issues/3) for more information on planned asset types, implementation status, and please contribute to the discussion if you think something is missing.

Currently supported assets:
- ✅ `sass`: Trunk ships with a built-in sass/scss compiler. Just link to your sass files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control.
- ✅ `css`: Trunk will copy linked css files found in the source HTML without content modification. This content is hashed for cache control.
  - In the future, Trunk will resolve local `@imports`, will handle minification (see [#7](https://github.com/thedodd/trunk/issues/3)), and we may even look into a pattern where any CSS found in the source tree will be bundled, which would enable a nice zero-config "component styles" pattern. See [trunk#3](https://github.com/thedodd/trunk/issues/3) for more details.
- ✅ `icon`: Trunk will automatically copy referenced icons to the `dist` dir. This content is hashed for cache control.

### images
Other image types can be copied into the `dist` dir by adding a link like this to your source HTML: `<link rel="trunk-dist" href="path/to/resource"/>`. This will cause Trunk to find the target resource, and copy it to the `dist` dir unmodified. No hashing will be applied. The link itself will be removed from the HTML.

This will allow your WASM application to reference images directly from the `dist` dir, and Trunk will ensure that the images are available in the `dist` dir to be served. You will need to be sure to use the correct public URL in your code, which can be configured via the `--public-url` option for most Trunk commands.

---

### license
trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
