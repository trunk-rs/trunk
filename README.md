<h1 align="center">trunk</h1>
<div align="center">

[![Build Status](https://github.com/thedodd/trunk/workflows/ci/badge.svg?branch=master)](https://github.com/thedodd/trunk/actions)
[![Crates.io](https://img.shields.io/crates/v/trunk.svg)](https://crates.io/crates/trunk)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE)
![Crates.io](https://img.shields.io/crates/d/trunk.svg)

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
cargo install trunk

# Install wasm-bindgen-cli.
cargo install wasm-bindgen-cli
# NOTE: in the future, trunk will do this for you.
```

Get setup with your favorite `wasm-bindgen` based framework. [Yew](https://github.com/yewstack/yew) & [Seed](https://github.com/seed-rs/seed) are the most popular options today, but there are others. Trunk will work with any `wasm-bindgen` based framework. The easiest way to ensure that your application launches properly is to [setup your app as an executable](https://doc.rust-lang.org/cargo/guide/project-layout.html) with a standard `main` function:

```rust
fn main() {
    // ... your app setup code here ...
}
```

Trunk uses a source HTML file to drive all asset building and bundling. Trunk also ships with a [built-in sass/scss compiler](https://github.com/compass-rs/sass-rs), so let's get started with the following example. Copy this HTML to the root of your project's repo as `index.html`:

```html
<html>
  <head>
    <link rel="stylesheet" href="index.scss"/>
  </head>
</html>
```

`trunk build` will produce the following HTML at `dist/index.html`, along with the compiled scss, WASM & the JS loader for the WASM:

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
`trunk build` runs a cargo build targeting the wasm32 instruction set, runs `wasm-bindgen` on the built WASM, and spawns asset build pipelines for any assets defined in the target `index.html`.

Trunk leverages Rust's powerful concurrency primitives for maximum build speeds & throughput.

### watch
`trunk watch` does the same thing as `trunk build`, but also watches the filesystem for changes, triggering new builds as changes are detected.

### serve
`trunk serve` does the same thing as `trunk watch`, but also spawns a web server.

### clean
`trunk clean` cleans up any build artifacts generated from earlier builds.

## assets
Trunk is still a young project, and new asset types will be added as we move forward. Keep an eye on [trunk#3](https://github.com/thedodd/trunk/issues/3) for more information on planned asset types, implementation status, and please contribute to the discussion if you think something is missing.

Currently supported assets:
- ✅ `sass`: Trunk ships with a [built-in sass/scss compiler](https://github.com/compass-rs/sass-rs). Just link to your sass files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control.
- ✅ `css`: Trunk will copy linked css files found in the source HTML without content modification. This content is hashed for cache control.
  - In the future, Trunk will resolve local `@imports`, will handle minification (see [trunk#7](https://github.com/thedodd/trunk/issues/3)), and we may even look into a pattern where any CSS found in the source tree will be bundled, which would enable a nice zero-config "component styles" pattern. See [trunk#3](https://github.com/thedodd/trunk/issues/3) for more details.
- ✅ `icon`: Trunk will automatically copy referenced icons to the `dist` dir. This content is hashed for cache control.
- ✅ `js snippets`: [wasm-bindgen JS snippets](https://rustwasm.github.io/docs/wasm-bindgen/reference/js-snippets.html) are automatically copied to the dist dir, hashed and ready to rock.

### images & other resources
Images and other resource types can be copied into the `dist` dir by adding a link like this to your source HTML: `<link rel="trunk-dist" href="path/to/resource"/>` (note the `rel="trunk-dist"` attribute). This will cause Trunk to find the target resource, and copy it to the `dist` dir unmodified. No hashing will be applied. The link itself will be removed from the HTML.

This will allow your WASM application to reference images directly from the `dist` dir, and Trunk will ensure that the images are available in the `dist` dir to be served. You will need to be sure to use the correct public URL in your code, which can be configured via the `--public-url` CLI flag to Trunk.

**NOTE:** as Trunk continues to mature, we will find better ways to include images and other resources. Hashing content for cache control is great, we just need to find a nice pattern to work with images referenced in Rust components. Please contribute to the discussion over in [trunk#9](https://github.com/thedodd/trunk/issues/9)! See you there.

## configuration
Trunk supports a layered config system. At the base, a config file can encapsulate project specific defaults, paths, ports and other config. Environment variables can be used to overwrite config file values. Lastly, CLI arguments / options take final precedence.

### Trunk.toml
Trunk supports an optional `Trunk.toml` config file. An example config file is included [in the Trunk repo](https://github.com/thedodd/trunk/blob/master/Trunk.toml), and shows all available config options along with their default values. By default, Trunk will look for a `Trunk.toml` config file in the current working directory. Trunk supports the global `--config` option to specify an alternative location for the file.

### environment variables
Trunk environment variables mirror the `Trunk.toml` config schema. All Trunk environment variables have the following 3 part form `TRUNK_<SECTION>_<ITEM>`, where `TRUNK_` is the required prefix, `<SECTION>` is one of the `Trunk.toml` sections, and `<ITEM>` is a specific configuration item from the corresponding section. E.G., `TRUNK_SERVE_PORT=80` will cause `trunk serve` to listen on port `80`. The equivalent CLI invokation would be `trunk serve --port=80`.

### cli arguments & options
The final configuration layer is the CLI itself. Any arguments / options provided on the CLI will take final precedence over any other config layer.

## contributing
Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](./CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use Github's draft pull request feature.

---

### license
trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
