+++
title = "Trunk"
description = "a WASM web application bundler for Rust"
weight = 1
+++

This text is in addition to [commands](@/commands.md).

The `trunkrs.dev` software  solves the problem of
collecting, _bundling_, various components for a webfrontend.
Such as HTML, pictures and Javascript.
And that Javascript is generated from other source files.
It is that _generating_, that `building` what `trunk` triggers.

# What is it

`trunk` can be a _daemon_, `trunk` can be a _builder_.
Upon startup you decide what it is.

# Builder

For a succesful `trunk build` there needs to be
at least a `index.html`.

Produced artifacts go into `dist/` directory.



# Daemon

`trunk serve` is intented for speeding up the development cycle.
You get an extra HTTP server for the benefit of
