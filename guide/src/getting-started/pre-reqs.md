# Pre-requisites

While `trunk` tries to fetch tools automatically as needed (unless you're running with `--offline`), some
pre-requisites may be required, depending on your environment.

## Rust

It might be obvious, but `trunk` requires an installation of Rust. Not only when installing `trunk` itself from sources,
but also for compiling the Rust-based projects to WebAssembly.

The instructions of installing Rust may vary based on your operating system, a reasonable default comes from the Rust
project: <https://www.rust-lang.org/learn/get-started>

Once installed, you should have the following tools available on your command line:

* `rustup`
* `cargo`

## WebAssembly target

By default, the Rust installation will only install the target for your current machine. However, in this case, we want
to cross-compile to WebAssembly. Therefore, it is required to install the target `wasm32-unknown-unknown`. Assuming
you have installed Rust using the standard process and can use `rustup`, you can add the target using:

```shell
rustup target add wasm32-unknown-unknown
```
