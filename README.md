# Trunk

[![Build Status](https://github.com/trunk-rs/trunk/actions/workflows/ci.yaml/badge.svg)](https://github.com/trunk-rs/trunk/actions)
[![](https://img.shields.io/crates/v/trunk.svg?color=brightgreen&style=flat-square)](https://crates.io/crates/trunk)
![](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square)
[![Discord Chat](https://img.shields.io/discord/793890238267260958?logo=discord&style=flat-square)](https://discord.gg/JEPdBujTDr)
[![](https://img.shields.io/crates/d/trunk?label=downloads%20%28crates.io%29&style=flat-square)](https://crates.io/crates/trunk)
[![](https://img.shields.io/github/downloads/trunk-rs/trunk/total?label=downloads%20%28GH%29&style=flat-square)](https://github.com/trunk-rs/trunk/releases)
![](https://img.shields.io/homebrew/installs/dy/trunk?color=brightgreen&label=downloads%20%28brew%29&style=flat-square)

**Build, bundle & ship your Rust WASM application to the web.**
<br/>
*”Pack your things, we’re going on an adventure!” ~ Ferris*

Trunk is a WASM web application bundler for Rust. Trunk uses a simple, optional-config pattern for building & bundling WASM, JS snippets & other assets (images, css, scss) via a source HTML file.

**📦 Dev server** - Trunk ships with a built-in server for rapid development workflows, as well as support for HTTP & WebSocket proxies.

**🏗 Change detection** - Trunk watches your application for changes and triggers builds for you, including automatic browser reloading.

## Getting Started

Head on over to the [Trunk website](https://trunkrs.dev), everything you need is there. A few quick links:

- [Install](https://trunkrs.dev/#install)
  - Download a released binary: https://github.com/trunk-rs/trunk/releases
  - `cargo binstall trunk` (installing a pre-compiled binary using [cargo-binstall](https://github.com/cargo-bins/cargo-binstall))
  - `cargo install trunk --locked` (compile your own binary from crates.io)
  - `cargo install --git https://github.com/trunk-rs/trunk trunk` (compile your own binary from the most recent git commit)
  - `cargo install --path . trunk` (compile your own binary form your local source)
  - `brew install trunk` (installing from [Homebrew](https://brew.sh/))
  - `nix-shell -p trunk` (installing from [nix packages](https://nixos.org/))
- [App Setup](https://trunkrs.dev//#app-setup)
- [Assets](https://trunkrs.dev/assets/)
- [Configuration](https://trunkrs.dev/configuration/)
- [CLI Commands](https://trunkrs.dev/commands/)

## Examples

Check out the example web applications we maintain in-repo under the `examples` directory.

## Contributing

Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](./CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use GitHub's draft pull request feature.

### License

trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
