# Example browser tests

Build the CLI, then from the repository root run:

```sh
TRUNK_REQUIRED_VERSION='*' target/debug/trunk --config=examples/vanilla build --release
npm ci --prefix examples/ci
cd examples/ci
npx playwright install --with-deps chromium
EXAMPLE=vanilla npm test
```

The tests host the selected example's existing `dist` directory on port 8080
and validate its core behavior in Chromium. They never invoke `trunk serve`.
The static host provides isolation headers needed by shared-memory WASM.
The `yew-tls` application is tested over HTTP like the other static builds;
Trunk's TLS and proxy server features are outside this suite. CI still builds
`proxy` in release mode, but skips its browser test.

CI caches npm downloads, Chromium and Trunk tool downloads alongside Cargo
caches. Failed tests retain traces, screenshots and browser logs under
`test-results`. Server output appears in the job log. External browser resources
are replaced with empty responses to avoid depending on CDN availability.
