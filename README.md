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

Serve the contents of your `dist` dir, and you're ready to rock.

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
