+++
title = "Trunk"
sort_by = "weight"
+++

Trunk is a WASM web application bundler for Rust. Trunk uses a simple, optional-config pattern for building & bundling WASM, JS snippets & other assets (images, css, scss) via a source HTML file.

# Getting Started
## Install
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

Any additional tools like `wasm-bindgen` and `wasm-opt` are automatically downloaded and managed by trunk. Therefore, no further steps required 🎉.

## App Setup
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
    <link rel="stylesheet" href="/index-c920ca43256fdcb9.css">
    <link rel="preload" href="/index-7eeee8fa37b7636a_bg.wasm" as="fetch" type="application/wasm" crossorigin="">
    <link rel="modulepreload" href="/index-7eeee8fa37b7636a.js">
  </head>
  <body>
    <script type="module">
      import init from '/index-7eeee8fa37b7636a.js';
      init('/index-7eeee8fa37b7636a_bg.wasm');
    </script>
  </body>
</html>
```

The contents of your `dist` dir are now ready to be served on the web.

# Next Steps
That's not all! Trunk has even more useful features. Head on over to the following sections to learn more about how to use Trunk effectively.

- [Assets](@/assets.md): learn about all of Trunk's supported asset types.
- [Configuration](@/configuration.md): learn about Trunk's configuration system and how to use the Trunk proxy.
- [Commands](@/commands.md): learn about Trunk's CLI commands for use in your development workflows.
- Join us on Discord by following this link <a href="https://discord.gg/JEPdBujTDr"><img src="https://img.shields.io/discord/793890238267260958?logo=discord&style=flat-square" style="vertical-align:text-top;" alt="Discord Chat"/></a>

# Contributing
Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](https://github.com/thedodd/trunk/blob/master/CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use GitHub's draft pull request feature.

# License
<p>
    <span><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square"/></span>
    <br/>
    trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
</p>
