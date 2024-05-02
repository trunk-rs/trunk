# Startup event

The initializer code snippet of Trunk will emit an event when the WebAssembly application has been loaded and started.

```admonish note
This event is independent of the initializer functionality.
```

## Definition

The event is called `TrunkApplicationStarted` and is executed after the WebAssembly has been loaded and initialized.

The event will have custom details:

```javascript
{
  wasm // The web assembly instance
}
```

## Example

The following snippet can be used to run code after the initialization of the WebAssembly application:

```html
<script type="module">
  addEventListener("TrunkApplicationStarted", (event) => {
  console.log("application started - bindings:", window.wasmBindings, "WASM:", event.detail.wasm);
  // wasm_ffi is a function exported from WASM to JavaScript
  window.wasmBindings.wasm_ffi();
  // You can also run this via the WASM instance in the details
  // event.detail.wasm.wasm_ffi();
});
</script>
```

Also see the vanilla example: <https://github.com/trunk-rs/trunk/tree/main/examples/vanilla>.
