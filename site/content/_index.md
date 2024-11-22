+++
title = "Trunk"
sort_by = "weight"
+++

Trunk is a WASM web application bundler for Rust. Trunk uses a simple, optional-config pattern for building & bundling WASM, JS snippets & other assets (images, css, scss) via a source HTML file.

# Getting Started

## Install

First, install Trunk via one of the following options.

### Plain cargo

Download the sources and build them yourself:

```bash
cargo install --locked trunk
```

You can also toggle some features using the `--features` flag:

<dl>
<dt><code>rustls</code></dt><dd>Use rustls for client and server sockets</dd>
<dt><code>native-tls</code> (default)</dt><dd>Enable the use of the system native TLS stack for client sockets, and `openssl` for server sockets</dd>
<dt><code>update_check</code> (default)</dt><dd>Enable the update check on startup</dd>
</dl>

**NOTE:** If both `rustls` and `native-tls` are enabled, `rustls` will be used. You can disable the default `rustls` using
`--no-default-features`.

### Cargo binstall

You can download a released binary from GitHub releases through [`binstall`](https://github.com/cargo-bins/cargo-binstall).

```bash
cargo binstall trunk
```

### GitHub release download

Fetch and unpack a released binary from the [release page](https://github.com/trunk-rs/trunk/releases).

For example (be sure to check for the most recent version):

```bash
wget -qO- https://github.com/trunk-rs/trunk/releases/download/0.17.10/trunk-x86_64-unknown-linux-gnu.tar.gz | tar -xzf-
```

### NixOS

```bash
nix-env -i trunk
```

### Brew

```bash
brew install trunk
```

## Additional tools

Any additional tools like `wasm-bindgen` and `wasm-opt` are automatically downloaded and managed by trunk. Therefore, no further steps required 🎉.

**Note:** Until `wasm-bindgen` has pre-built binaries for Apple M1, M1 users will need to install `wasm-bindgen` manually.

```bash
cargo install --locked wasm-bindgen-cli
```

## App Setup

Any `wasm-bindgen`-based framework will work with Trunk. If you're new to [frontend development in Rust][], [Yew][] and [Leptos][] are two popular options.

[frontend development in Rust]: https://github.com/flosse/rust-web-framework-comparison#frontend-frameworks-wasm
[Yew]: https://yew.rs/
[Leptos]: https://leptos.dev/

The easiest way to ensure that your application launches properly is to [setup your app as an executable][cargo-layout] with a standard `main` function:

[cargo-layout]: https://doc.rust-lang.org/cargo/guide/project-layout.html

```rust
fn main() {
    // ... your app setup code here ...
}
```

Trunk uses a source HTML file to drive all asset building and bundling. Trunk also uses the official [dart-sass](https://github.com/sass/dart-sass), so let's get started with the following example. Copy this HTML to the root of your project's repo as `index.html`:

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
      import init, * as bindings from '/index-7eeee8fa37b7636a.js';
      window.wasmBindings = bindings;
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
- [Advanced topics](@/advanced.md): learn about some more advanced topics.
- Join us on Discord by following this link [![](https://img.shields.io/discord/793890238267260958?logo=discord&style=flat-square "Discord Chat")](https://discord.gg/JEPdBujTDr)

# Contributing

Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](https://github.com/trunk-rs/trunk/blob/main/CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use GitHub's draft pull request feature.

# License

<span><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" alt="license badge"/></span>
<br>
trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
