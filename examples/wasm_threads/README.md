# Web Workers with shared memory, using only a single WASM binary

This is a port of the [wasm_threads `simple.rs` example](https://github.com/chemicstry/wasm_thread/tree/main?tab=readme-ov-file#simple).

It should also work similarly with `wasm-bindgen-rayon` and other packages that use SharedArrayBuffer.

An explanation of this approach is described [here](https://rustwasm.github.io/wasm-bindgen/examples/raytrace.html).

## Limitations

It has some significant advantages over the `webworker*' examples, but also some significant disadvantages.

For starters, it requires cross-site isolation (setting 2 headers), which is [required for this approach to workers](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer#security_requirements).
We've added it to the trunk config.

These same headers are also required for deployment. Github Pages does not allow setting headers, and alternatives such as using `<meta>` did not work in my tests, so these sites can't be deployed that way. Cloudflare Pages is a free alternative that does allow headers, and it worked for me.

It also requires nightly Rust, because [the standard library needs to be rebuilt](https://github.com/RReverser/wasm-bindgen-rayon?tab=readme-ov-file#building-rust-code).

Some libraries may not work correctly in this way, as they were written assuming the historical single-threaded wasm32 runtimes. wgpu` has the `fragile-send-sync-non-atomic-wasm` flag which, if set, will not work with this. For example, `egui` currently sets this flag, although it can be removed with a manual, not well tested [patch](https://github.com/9SMTM6/egui/commit/11b00084e34c8b0ff40bac82274291dff64c26db).

Additional restrictions are listed [here](https://rustwasm.github.io/wasm-bindgen/examples/raytrace.html#caveats) (some of which may be solved or worked around in libraries). Limitations specific to `wasm_thread' are explained in the source code comments.

## Advantages

* Code sharing
  * Improves developer experience
  * Also means that the WASM binary is shared, which in extreme cases can be half the size of the deployed application.
  * Avoids the [web-worker cache-invalidation issue](https://github.com/trunk-rs/trunk/issues/405)
* Memory sharing between threads
  * can be a huge performance gain


## Notes on applying this

Note that this requires the [toolchain file](./rust-toolchain.toml) and the [cargo config](.cargo/config.toml).

The `_headers` file and its copy in `index.html` is simply an example of how to set the headers using Cloudflare Pages.

If you receive errors such as

> [Firefox] The WebAssembly.Memory object cannot be serialized. The Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy HTTP headers can be used to enable this.

> [Chrome] SharedArrayBuffer transfer requires self.crossOriginIsolated.

Then the headers are not set correctly. You can check the response headers on the `/` file in the Network tab of the browser developer tools.

Errors such as
> InternalError: too much recursion

Were solved in the example by enabling optimizations. It was sufficient to do this on dependencies only, with

```
# in Cargo.toml
# Optimize all dependencies even in debug builds:
[profile.dev.package."*"]
opt-level = 2
```

This will slow down clean rebuilds, but should not affect rebuild speed during normal development.

## Using rust-analyzer

Since we use the build-std flag in the toolchain file, and that requires an explicit target to be set for compilation etc., this will break rust-analyzer in many setups. This can be solved by specifying an explicit target for the workspace, such as with the provided [config file for vscode](./.vscode/settings.json).
