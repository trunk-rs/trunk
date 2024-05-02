
# Library crate

Aside from having a `main` function, it is also possible to up your project as a `cdylib` project. In order to do that,
add the following to your `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

And then, define the entrypoint in your `lib.rs` like (does not need to be `async`):

```rust
#[wasm_bindgen(start)]
pub async fn run() {}
```
