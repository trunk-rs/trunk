+++
title = "Commands"
description = "Commands"
weight = 3
+++

Trunk ships with a set of CLI commands to help you in your development workflows.

# build
`trunk build` runs a cargo build targeting the wasm32 instruction set, runs `wasm-bindgen` on the built WASM, and spawns asset build pipelines for any assets defined in the target `index.html`.

Trunk leverages Rust's powerful concurrency primitives for maximum build speeds & throughput.

# watch
`trunk watch` does the same thing as `trunk build`, but also watches the filesystem for changes, triggering new builds as changes are detected.

# serve
`trunk serve` does the same thing as `trunk watch`, but also spawns a web server.

# clean
`trunk clean` cleans up any build artifacts generated from earlier builds.

# config show
`trunk config show` prints out Trunk's current config, before factoring in CLI arguments. Nice for testing & debugging.

# tools show
`trunk tools show` prints out information about tools required by trunk and the project. It shows which tools are expected and which are found. 
