<h1 align="center">Trunk</h1>
<div align="center">

[![Build Status](https://github.com/thedodd/trunk/actions/workflows/ci.yaml/badge.svg?branch=master)](https://github.com/thedodd/trunk/actions)
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

Trunk is a WASM web application bundler for Rust. Trunk uses a simple, optional-config pattern for building & bundling WASM, JS snippets & other assets (images, css, scss) via a source HTML file.

**NOTE:** This is a forked version of `trunk`, adding features and bug fixes which didn't get merged into trunk so far.
Replace `trunk` with `trunk-ng` for all operations.

**📦 Dev server** - Trunk ships with a built-in server for rapid development workflows, as well as support for HTTP & WebSocket proxies.

**🏗 Change detection** - Trunk watches your application for changes and triggers builds for you. Browser reloading, HMR, and other related features are in-progress.

## Getting Started
Head on over to the [Trunk website](https://trunkrs.dev), everything you need is there. A few quick links:
- [Install](https://trunkrs.dev/#install)
- [App Setup](https://trunkrs.dev/#app-setup)
- [Assets](https://trunkrs.dev/assets/)
- [Configuration](https://trunkrs.dev/configuration/)
- [CLI Commands](https://trunkrs.dev/commands/)

## Examples
Check out a few of the example web applications we maintain in-repo:
- [Yew + ybc](./examples/yew/README.md): demo app built using [Yew](https://yew.rs) & [ybc](https://github.com/thedodd/ybc).
- [Seed](./examples/seed/README.md): demo app built using [Seed](https://seed-rs.org).
- [Vanilla](./examples/vanilla/README.md): demo app built using plain old web-sys.

## Contributing
Anyone and everyone is welcome to contribute! Please review the [CONTRIBUTING.md](./CONTRIBUTING.md) document for more details. The best way to get started is to find an open issue, and then start hacking on implementing it. Letting other folks know that you are working on it, and sharing progress is a great approach. Open pull requests early and often, and please use Github's draft pull request feature.

---

### License
trunk is licensed under the terms of the MIT License or the Apache License 2.0, at your choosing.
