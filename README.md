<h1 align="center">trunk</h1>
<div align="center">

[![Build Status](https://github.com/thedodd/trunk/workflows/ci/badge.svg?branch=master)](https://github.com/thedodd/trunk/actions)
[![](https://img.shields.io/crates/v/trunk.svg?color=brightgreen&style=flat-square)](https://crates.io/crates/trunk)
[![](https://img.shields.io/homebrew/v/trunk?color=brightgreen&style=flat-square)](https://formulae.brew.sh/formula/trunk)
![](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)
[![Discord Chat](https://img.shields.io/discord/793890238267260958?logo=discord&style=flat-square)](https://discord.gg/JEPdBujTDr)
![](https://img.shields.io/crates/d/trunk?label=downloads%20%28crates.io%29&style=flat-square)
![](https://img.shields.io/github/downloads/thedodd/trunk/total?label=downloads%20%28GH%29&style=flat-square)
![](https://img.shields.io/homebrew/installs/dy/trunk?color=brightgreen&label=downloads%20%28brew%29&style=flat-square)

  <strong>
    Build, bundle & ship your Rust WASM application to the web.
  </strong>
  <br/>
  <i>
    ”Pack your things, we’re going on an adventure!” ~ Ferris
  </i>
</div>
<br/>

Trunk is a WASM web application bundler for Rust. Trunk uses a simple, zero-config pattern for building & bundling WASM, JS snippets & other assets (images, css, scss) via a source HTML file.

## getting started
### install
First, install Trunk via one of the following options.
```bash
# Install via homebrew on Mac, Linux or Windows (WSL).
brew install trunk

# Install a release binary (great for CI).
# You will need to specify a value for ${VERSION}.
wget -qO- https://github.com/thedodd/trunk/releases/download/${VERSION}/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-

# Install via cargo.
cargo install --locked trunk
```
<small>Release binaries can be found on the [Github releases page](https://github.com/thedodd/trunk/releases).</small>

Next, we will need to install `wasm-bindgen-cli`. In the future Trunk will handle this for you.
```
cargo install wasm-bindgen-cli
```

If using wasm-opt, we will need to install `wasm-opt` which is part of `binaryen`. On MacOS we can install it with Homebrew:

```sh
brew install binaryen
```

Some linux distributions provide a `binaryen` package in their package managers but if it's not available, outdated or you're on Windows, then we can grab a [pre-compiled release](https://github.com/WebAssembly/binaryen/releases), extract it and put the `wasm-opt` binary in some location that's available on the PATH.

### app setup
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
    <link data-trunk rel="scss" href="path/to/index.scss"/>
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

### config show
`trunk config show` prints out Trunk's current config, before factoring in CLI arguments. Nice for testing & debugging.

## assets
Declaring assets to be processed by Trunk is simple and extensible. All assets to be processed by Trunk must follow these three rules:
- must be declared as a valid HTML `link` tag.
- must have the attribute `data-trunk`.
- must have the attribute `rel="{type}"`, where `{type}` is one of the asset types listed below.

This will typically look like: `<link data-trunk rel="{type}" href="{path}" ..other options here.. />`. Each asset type described below specifies the required and optional attributes for its asset type. All `<link data-trunk .../>` HTML elements will be replaced with the output HTML of the associated pipeline.

Currently supported asset types:
- ✅ `rust`: Trunk will compile the specified Cargo project as the main WASM application. This is optional. If not specified, Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `href`: (optional) the path to the `Cargo.toml` of the Rust project. If a directory is specified, then Trunk will look for the `Cargo.toml` in the given directory. If no value is specified, then Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `data-bin`: (optional) the name of the binary to compile and use as the main WASM application. If the Cargo project has multiple binaries, this value will be required for proper functionality.
  - `data-wasm-opt`: (optional) run wasm-opt with the set optimization level. wasm-opt is **turned off by default** but that may change in the future. The possible values are `0`, `1`, `2`, `3`, `4`, `s`, `z` or an _empty value_ for wasm-opt's default. Set this option to `0` to disable wasm-opt explicitly. The values `1-4` are increasingly stronger optimization levels for speed. `s` and `z` (z means more optimization) optimize for binary size instead.
  - `data-keep-debug`: (optional) instruct `wasm-bindgen` to preserve debug info in the final WASM output, even for `--release` mode.
  - `data-no-demangle`: (optional) instruct `wasm-bindgen` to not demangle Rust symbol names.
- ✅ `sass`, `scss`: Trunk ships with a [built-in sass/scss compiler](https://github.com/compass-rs/sass-rs). Just link to your sass files from your source HTML, and Trunk will handle the rest. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the sass/scss file to be processed.
- ✅ `css`: Trunk will copy linked css files found in the source HTML without content modification. This content is hashed for cache control. The `href` attribute must be included in the link pointing to the css file to be processed.
  - In the future, Trunk will resolve local `@imports`, will handle minification (see [trunk#7](https://github.com/thedodd/trunk/issues/3)), and we may even look into a pattern where any CSS found in the source tree will be bundled, which would enable a nice zero-config "component styles" pattern. See [trunk#3](https://github.com/thedodd/trunk/issues/3) for more details.
- ✅ `icon`: Trunk will copy the icon image specified in the `href` attribute to the `dist` dir. This content is hashed for cache control.
- ✅ `inline`: Trunk will inline the content of the file specified in the `href` attribute into `index.html`. This content is copied exactly, no hashing is performed.
  - `type`: (optional) either `html`, `css`, or `js`. If not present, the type is inferred by the file extension. `css` is wrapped in `style` tags, while
  `js` is wrapped in `script` tags.
- ✅ `copy-file`: Trunk will copy the file specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.
- ✅ `copy-dir`: Trunk will recursively copy the directory specified in the `href` attribute to the `dist` dir. This content is copied exactly, no hashing is performed.
- ⏳ `rust-worker`: (in-progress) Trunk will compile the specified Rust project as a WASM web worker. The following attributes are required:
  - `href`: (optional) the path to the `Cargo.toml` of the Rust project. If a directory is specified, then Trunk will look for the `Cargo.toml` in the given directory. If no value is specified, then Trunk will look for a `Cargo.toml` in the parent directory of the source HTML file.
  - `data-bin`: (optional) the name of the binary to compile and use as the web worker. If the Cargo project has multiple binaries, this value will be required for proper functionality.

Trunk is still a young project, and new asset types will be added as we move forward. Keep an eye on [trunk#3](https://github.com/thedodd/trunk/issues/3) for more information on planned asset types, implementation status, and please contribute to the discussion if you think something is missing.

### js snippets
JS snippets generated from the [wasm-bindgen JS snippets feature](https://rustwasm.github.io/docs/wasm-bindgen/reference/js-snippets.html) are automatically copied to the dist dir, hashed and ready to rock. No additional setup is required. Just use the feature in your application, and Trunk will take care of the rest.

### images & other resources
Images and other resource types can be copied into the `dist` dir by adding a link like this to your source HTML: `<link data-trunk rel="copy-file" href="path/to/image"/>`. Any normal file type is supported. This will cause Trunk to find the target resource, and copy it to the `dist` dir unmodified. No hashing will be applied. The link itself will be removed from the HTML. To copy an entire directory of assets/images, you can use the following HTML: `<link data-trunk rel="copy-dir" href="path/to/images-dir"/>`.

This will allow your WASM application to reference images directly from the `dist` dir, and Trunk will ensure that the images are available in the `dist` dir to be served.

**NOTE:** as Trunk continues to mature, we will find better ways to include images and other resources. Hashing content for cache control is great, we just need to find a nice pattern to work with images referenced in Rust components. Please contribute to the discussion over in [trunk#9](https://github.com/thedodd/trunk/issues/9)! See you there.

### directives
You can instruct Trunk to write the URL passed to `--public-url` to the HTML output by adding this to your `<head>`: `<base data-trunk-public-url/>`.
Trunk will set the `href` attribute of the element to the public URL. This changes the behavior of relative URLs to be relative to the public URL instead of the current location.
You can also access this value at runtime using `document.baseURI` which is useful for apps that need to know what base URL they're hosted on (eg. for routing).

## configuration
Trunk supports a layered config system. At the base, a config file can encapsulate project specific defaults, paths, ports and other config. Environment variables can be used to overwrite config file values. Lastly, CLI arguments / options take final precedence.

### Trunk.toml
Trunk supports an optional `Trunk.toml` config file. An example config file is included [in the Trunk repo](https://github.com/thedodd/trunk/blob/master/Trunk.toml), and shows all available config options along with their default values. By default, Trunk will look for a `Trunk.toml` config file in the current working directory. Trunk supports the global `--config` option to specify an alternative location for the file.

Note that any relative paths declared in a `Trunk.toml` file will be treated as being relative to the `Trunk.toml` file itself.

### environment variables
Trunk environment variables mirror the `Trunk.toml` config schema. All Trunk environment variables have the following 3 part form `TRUNK_<SECTION>_<ITEM>`, where `TRUNK_` is the required prefix, `<SECTION>` is one of the `Trunk.toml` sections, and `<ITEM>` is a specific configuration item from the corresponding section. E.G., `TRUNK_SERVE_PORT=80` will cause `trunk serve` to listen on port `80`. The equivalent CLI invocation would be `trunk serve --port=80`.

### cli arguments & options
The final configuration layer is the CLI itself. Any arguments / options provided on the CLI will take final precedence over any other config layer.

## proxy
Trunk ships with a built-in proxy which can be enabled when running `trunk serve`. There are two ways to configure the proxy, each discussed below. All Trunk proxies will transparently pass along the request body, headers, and query parameters to the proxy backend.

### proxy cli flags
The `trunk serve` command accepts two proxy related flags.

`--proxy-backend` specifies the URL of the backend server to which requests should be proxied. The URI segment of the given URL will be used as the path on the Trunk server to handle proxy requests. E.G., `trunk serve --proxy-backend=http://localhost:9000/api/` will proxy any requests received on the path `/api/` to the server listening at `http://localhost:9000/api/`. Further path segments or query parameters will be seamlessly passed along.

`--proxy-rewrite` specifies an alternative URI on which the Trunk server is to listen for proxy requests. Any requests received on the given URI will be rewritten to match the URI of the proxy backend, effectively stripping the rewrite prefix. E.G., `trunk serve --proxy-backend=http://localhost:9000/ --proxy-rewrite=/api/` will proxy any requests received on `/api/` over to `http://localhost:9000/` with the `/api/` prefix stripped from the request, while everything following the `/api/` prefix will be left unchanged.

`--proxy-ws` specifies that the proxy is for a WebSocket endpoint.

### config file
The `Trunk.toml` config file accepts multiple `[[proxy]]` sections, which allows for multiple proxies to be configured. Each section requires at least the `backend` field, and optionally accepts the `rewrite` and `ws` fields, corresponding to the `--proxy-*` CLI flags discussed above.

As it is with other Trunk config, a proxy declared via CLI will take final precedence and will cause any config file proxies to be ignored, even if there are multiple proxies declared in the config file.

The following is a snippet from the `Trunk.toml` file in this repo:

```toml
[[proxy]]
rewrite = "/api/v1/"
backend = "http://localhost:9000/"
```

## contributing
Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](./CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use Github's draft pull request feature.

---

### license
trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
