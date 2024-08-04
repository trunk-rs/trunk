# Support workers with shared memory and one wasm binary

This is a port of [wasm_threads `simple.rs` example](https://github.com/chemicstry/wasm_thread/tree/main?tab=readme-ov-file#simple).

It should also work similarly with `wasm-bindgen-rayon` and `wasm-mt`.

## Limitations

It has a few considerable advantages over the `webworker*` examples, but also considerable disadvantages.

For starters, `trunk serve` won't work currently, as it doesn't do Cross Site Isolation (setting 2 headers), which is [required for this approach to workers](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/SharedArrayBuffer#security_requirements).
`sfz` works, with `cargo install sfz` and run `sfz dist --coi` in the root directory of this example, then click on index.html in the website directory.

These same headers are also required during deployment. Github pages does not allow setting headers, and alternatives such as using `<meta>` did not work in my testing, so these sites can't be deployed like that. Cloudflare Pages is a free alternative that allows setting headers that worked for me.

Then it also requires nightly Rust, because [the standard library needs to be rebuild](https://github.com/RReverser/wasm-bindgen-rayon?tab=readme-ov-file#building-rust-code).

Additional limitations are listed [here](https://github.com/chemicstry/wasm_thread/tree/7bc7dccebf2d96775b60bf0fda6b4173aa993aff?tab=readme-ov-file#notes-on-wasm-limitations).

## Advantages

* code sharing
  * improves dev experience
  * also means that the WASM binary will be shared, which can in the extreme case half the size of the website.
* shared memory between threads
  * can be a huge performance win

## Notes on applying this

Note that this requires the [toolchain file](./rust-toolchain.toml) and the [cargo config](.cargo/config.toml).

The `_headers` file and its copy in `index.html` is simply an example of how to set the headers using Cloudflare Pages.
