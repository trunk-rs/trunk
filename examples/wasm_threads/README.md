# Support workers with shared memory and one wasm binary

This is a port of [wasm_threads `simple.rs` example](https://github.com/chemicstry/wasm_thread/tree/main?tab=readme-ov-file#simple).

It should also work similarly with `wasm-bindgen-rayon` and other packages that use SharedArrayBuffer.

An explanation of that approach is described [here](https://rustwasm.github.io/wasm-bindgen/examples/raytrace.html)

## Limitations

It has a few considerable advantages over the `webworker*` examples, but also considerable disadvantages.

For starters, this needs Cross Site Isolation (setting 2 headers), which is [required for this approach to workers](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer#security_requirements).
We've added them in the trunk config.

These same headers are also required during deployment. Github pages does not allow setting headers, and alternatives such as using `<meta>` did not work in my testing, so these sites can't be deployed like that. Cloudflare Pages is a free alternative that allows setting headers that worked for me.

Then it also requires nightly Rust, because [the standard library needs to be rebuild](https://github.com/RReverser/wasm-bindgen-rayon?tab=readme-ov-file#building-rust-code).

Some libraries might not work correctly like that, since they are written under the assumption of the historially single threaded wasm32 runtimes. `wgpu` has the `fragile-send-sync-non-atomic-wasm` flag, which if set will not work with this. Eg. `egui` sets this flag currently, though it can be removed with a manual, not well tested [patch](https://github.com/9SMTM6/egui/commit/11b00084e34c8b0ff40bac82274291dff64c26db).

Additional limitations are listed [here](https://rustwasm.github.io/wasm-bindgen/examples/raytrace.html#caveats) (some of them might be solved or worked around in libraries). Specifically for `wasm_thread` limitations are explained in the comments in the source code.

## Advantages

* code sharing
  * improves dev experience
  * also means that the WASM binary will be shared, which can in the extreme case half the size of the website.
* shared memory between threads
  * can be a huge performance win

## Notes on applying this

Note that this requires the [toolchain file](./rust-toolchain.toml) and the [cargo config](.cargo/config.toml).

The `_headers` file and its copy in `index.html` is simply an example of how to set the headers using Cloudflare Pages.

If you get errors such as

> [Firefox] The WebAssembly.Memory object cannot be serialized. The Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy HTTP headers can be used to enable this.

> [Chrome] SharedArrayBuffer transfer requires self.crossOriginIsolated.

Then the headers did not set correctly. You can check the response headers on the `/` file in the network tab of the browser developer tools.

## Using rust-analyzer

Since we use the build-std flag in the toolchain file, and that requires an explicit target to be set for compilation etc., this will break rust-analyzer in many setups. This can be solved by specifying an explicit target for the workspace, such as with the provided [config file for vscode](./.vscode/settings.json).
