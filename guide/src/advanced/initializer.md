
# Initializer

Since: `0.19.0-alpha.1`.

Trunk supports tapping into the initialization process of the WebAssembly application. By
default, this is not active and works the same way as with previous versions.

The default process is that trunk injects a small JavaScript snippet, which imports the JavaScript loader generated
by `wasm_bindgen` and calls the `init` method. That will fetch the WASM blob and run it.

The downside of this is, that during this process, there's no feedback for the user. Neither when it takes a bit longer
to load the WASM file, nor when something goes wrong.

Now it is possible to tap into this process by setting `data-initializer` to a JavaScript module file. This module file
is required to (default) export a function, which returns the "initializer" instance. Here is an example:

```javascript
export default function myInitializer () {
  return {
    onStart: () => {
      // called when the loading starts
    },
    onProgress: ({current, total}) => {
      // the progress while loading, will be called periodically.
      // "current" will contain the number of bytes of the WASM already loaded
      // "total" will either contain the total number of bytes expected for the WASM, or if the server did not provide
      //   the content-length header it will contain 0.
    },
    onComplete: () => {
      // called when the initialization is complete (successfully or failed)
    },
    onSuccess: (wasm) => {
      // called when the initialization is completed successfully, receives the `wasm` instance
    },
    onFailure: (error) => {
      // called when the initialization is completed with an error, receives the `error`
    }
  }
};
```

For a full example, see: <https://github.com/trunk-rs/trunk/tree/main/examples/initializer>.
