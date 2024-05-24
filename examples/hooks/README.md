Trunk | Hooks
=========================
An example using different hooks and ideas to run code during the build.

Once you've installed Trunk, simply execute `trunk serve --open` from this example's directory, and you should see the
web application rendered in your browser and some output on the browser's console.

## Hook

There is one `xtask` based build hook. Check out [`cargo-xtask`](https://github.com/matklad/cargo-xtask) to get better
understanding of the pattern, as it's not an actual command you install.

The `xtask` is triggered by Trunk's hook system and has access to the `TRUNK_*` environment variables during the hook
execution.

# `build.rs`

You can also use a simple [`build.rs` build script](https://doc.rust-lang.org/cargo/reference/build-scripts.html). This
will be executed during the build of the main WASM application. It will be run on the host system though (not using
`wasm32-unknown-unknown`).

It does not have access to the Trunk env-vars, but does have access to all kind of `cargo` information, like the
features enabled during compilation. If you run this example with `trunk --features a`, you will see a different output
on the console.
