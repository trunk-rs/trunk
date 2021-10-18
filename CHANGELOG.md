changelog
=========
This changelog follows the patterns described here: https://keepachangelog.com/en/1.0.0/.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

## Unreleased

### added.
- Open autoreload websocket using wss when assets are served over a secure connection.

## 0.14.0
### added
- Trunk now includes a hooks system. This improves many use cases where another build tool is needed alongside trunk, by allowing trunk to be configured to call them at various stages during the build pipeline. It is configured under `[[hooks]]` in `Trunk.toml`. More information can be found in the [Assets](https://trunkrs.dev/assets/) section of the docs.
- Added `trunk serve` autoreload triggered over websocket that reloads the page when a change is detected. The `--no-autoreload` flag disables this feature.
- Added the optional `pattern_script` field to the `Trunk.toml` for overloading the template of initialization script.
- Added the optional `pattern_preload` field to the `Trunk.toml` for overloading the template of WASM preloading.
- Added the optional `pattern_params` field to the `Trunk.toml` for extending `pattern_script` and `pattern_preload` with additional values, including external files. Overloading these parameters allow users to use `trunk` with other frameworks [like this](https://github.com/ivanceras/sauron/tree/5208508a9675852334b7cc7624ba83fdb11edeb1/examples/progressive-rendering).

### changed
- Download and use the official `dart-sass` binary for SASS/SCSS to CSS compilation. This allows to always support the latest features and will allow to make Trunk available for futher platforms in the future as this removes the dependency on `sass-rs`.
- Proxied websockets now shut down immediately upon completion of streaming data in either direction, instead of waiting for completion of both directions.

## 0.13.1
- Fixed [#219](https://github.com/thedodd/trunk/issues/219): Preserve websocket message types when sending to the backend.

## 0.13.0
- Trunk has been fully cut over to Tokio@1.x.
- As part of the Tokio cut over, the Trunk network stack is now fully based on Axum.
- All download utilities have been made fully async. This will likely help us in the future as we continue to leverage this functionality more and more.
- Added a new CLI option `trunk clean -t/--tools` which will optionally clean/remove any cached tools used by Trunk, such as `wasm-bindgen` & `wasm-opt`. This may be useful if there are ever issues with old tools which need to be removed.
- Fixed [#198](https://github.com/thedodd/trunk/issues/198) which was a long-standing issue with the static file server. In essence, Trunk will now check the `accept` header of any `GET` request matching the static file server, and if the requested asset does not exist and the `accept` header allows either `*/*` or `text/html`, then return the `index.html`.
    - This is expected functionality for SPAs which are using client side routing.
    - This reduces the friction which has often been observed with Trunk where a user is expecting a 404 to be served when requesting a static image, CSS, or some other asset. With this update, 404s will now be returned as expected, and the `index.html` should only be returned for applicable cases.
- Added a new `proxy` example which demonstrates all functionality of the Trunk proxy system.
- Fixed [#209](https://github.com/thedodd/trunk/issues/209) where the default Rust App pipeline was causing wasm-opt to be used even for debug builds when the Rust App HTML link was being omitted.
- Closed [#168](https://github.com/thedodd/trunk/issues/158): RSS feed for blog.
- Isolated code used for version checking & formatting of version output for downloadable applications (wasm-bindgen & wasm-opt). Added unit tests to cover this logic.
- Fixed [#197](https://github.com/thedodd/trunk/issues/197) & [#175](https://github.com/thedodd/trunk/pull/175) where disabling wasm-opt for debug builds while keeping it enable for release builds was not possible without some hacking. Now, wasm-opt will only be used for release builds and only when enabled. This semantically matches cargo's behavior with optimizations in release mode.

## 0.12.1
### fixed
- When wasm-opt level is not set explicitly, do not invoke it at all for debug builds, and for release builds invoke with default optimization level.

## 0.12.0
### added
- Closed [#139](https://github.com/thedodd/trunk/issues/139): Download and manage external applications (namely `wasm-bindgen` and `wasm-opt`) automatically. If available in the right version, system installed binaries are used but if absent the right version is downloaded and installed. This allows to use trunk without the extra steps of downloading the needed binaries manually.
- Added an example application for using Trunk with a vanilla (no frameworks) Rust application.

### changed
- `wasm-opt` is now enabled by default (with default optimization level) as the binary is automatically downloaded and installed by trunk. It can still be disabled by setting `data-wasm-opt` to `0`.

## 0.11.0
### added
- Closed [#158](https://github.com/thedodd/trunk/issues/158): Support for inlining SASS/SCSS after compilation using the new `data-inline` attribute.
- Closed [#145](https://github.com/thedodd/trunk/issues/145): Preloading of the WASM and JS files are now added to the `<head>` element. This ensures that the code starts downloading as soon as possible, and can make your app start up a few seconds earlier on a slow network.
- Closed [#135](https://github.com/thedodd/trunk/pull/135): Allow users to specify `data-keep-debug` & `data-no-mangle` options on Rust build pipelines, which influence their corresponding `wasm-bindgen` CLI options.
- Added an example application for using Trunk with a vanilla (no frameworks) Rust application.

### fixed
- Fixed [#148](https://github.com/thedodd/trunk/issues/148): any changes detected under a `.git` path are now being ignored by default.
- Fixed [#163](https://github.com/thedodd/trunk/issues/163): allow using `copy-file` assets on files which do not have a file extension.
- Fixed [#60](https://github.com/thedodd/trunk/issues/60): Added `data-cargo-features` attribute for rust asset type.

## 0.10.0
### changed
- Completely removed `indicatif` from the code base, and now we are using the `tracing` crate. This has greatly simplified the code base. Only downside ... no more progress spinner. Per a fair bit of demand however, cargo output and output from other subprocess calls are now piped directly to stdout/stderr for better visibility into what is happening behind the scenes.

## 0.9.2
### fixed
- Fixed a bug where build pipeline errors were being hidden/masked on subsequent builds.

## 0.9.1
### fixed
- Fixed a bug related to the watch system, which would cause build loops if there was an error on the initial build.

## 0.9.0
### added
Added support for proxying WebSockets. This was a long-standing feature request. Due to changes upstream in the async-std/tide ecosystem, we are now able to properly support this. This will also unlock some nice features such as HMR via WebSockets, and other such niceties.

- Added the `--proxy-ws` CLI option for enabling WebSocket proxying on a CLI defined proxy.
- Added the `ws = true` field to the `Trunk.toml` `[[proxy]]` sections which will enable WebSocket proxying for proxies defined in the `Trunk.toml`.
- WASM files are now automatically optimized with `wasm-opt` to reduce the binary size. The optimization level can be set with the new `data-wasm-opt` argument of the `rust` asset link and`wasm-opt` binary is now required to be globally installed on the system when being used. E.G., `<link data-trunk rel="rust" [...] data-wasm-opt="4"/>`.

### fixed
- Closed [#81](https://github.com/thedodd/trunk/issues/81): this is no longer needed as we now have support for WebSockets. HTTP2 is still outstanding, but that will not be a blocker for use from the web.
- Closed [#95](https://github.com/thedodd/trunk/issues/95): fixed via a few small changes to precedence in routing.
- Closed [#53](https://github.com/thedodd/trunk/issues/53): we've now implemented support for proxying WebSockets.

## 0.8.3
### fixed
- Fixed [#133](https://github.com/thedodd/trunk/issues/133) where `watch` was infinitely looping on Windows
because the canonicalization path didn't match the un-canonicalized ignore list.

## 0.8.2 & 0.8.1
### fixed
- Fixed [#124](https://github.com/thedodd/trunk/issues/124) where path canonicalization was being performed on a path which did not yet exist, and as it turns out was already in canonical form.

## 0.8.0
### added
- Closed [#93](https://github.com/thedodd/trunk/issues/93): The `watch` and `serve` subcommands can now watch specific folder(s) or file(s) through the new `--watch <path>...` option. Thanks to @malobre for all of the work on this one.
- Inline the content of files directly into `index.html` with the new `inline` asset. There are three content types that can be inlined: `html`, `css`, and `js`. The type can be specified with the `type` attribute or is inferred by the file extension.

### fixed
- Closed [#49](https://github.com/thedodd/trunk/issues/49): old artifacts in the dist dir are now being cleaned-up as new builds successfully complete. Thanks @philip-peterson & @hamza1311 for their work on this one.
- Fixed infinite rebuild loop on Windows started by `watch` command by path canonicalizing in the ignored paths resolver.

## 0.7.4
### fixed
- Fixed a regression in Trunk CLI help output, where incorrect help info was being displayed.

## 0.7.3
### fixed
- Closed [#82](https://github.com/thedodd/trunk/issues/82): Remove the hardcoded Unix (`/`) path separator from the code and convert Windows NT UNC path to its simplified alternative before passing to `cargo metadata` command to prevent issues with Rust package collisions and writing `index.html` file.
- Updated the `WatchSystem` to use `{:?}` debug formatting for errors to ensure that full error chains are reported. This impacts the `watch` & `serve` subcommands. The `build` command was already behaving as needed.

## 0.7.2
### fixed
- Closed [#78](https://github.com/thedodd/trunk/issues/78): Ensure all asset pipelines are properly rooted to the configured `public-url`, which defaults to `/`.

## 0.7.1
### fixed
- Closed [#76](https://github.com/thedodd/trunk/issues/76): Ensure canonical paths are used for pertinent paths in the runtime config to ensure watch config is able to properly ignore the dist dir and such.

## 0.7.0
- Made a lot of refactoring progress to be able to support [#28](https://github.com/thedodd/trunk/issues/28) & [#46](https://github.com/thedodd/trunk/issues/46). More work remains to be done, but the foundation to be able to knock these items out is now in place.

### changed
- All assets which are to be processed by trunk must now be declared as HTML `link` elements as such: `<link data-trunk rel="rust|sass|css|icon|copy-file|..." data-attr0 data-attr1/>`. The links may appear anywhere in the HTML and Trunk will process them and replace them or delete them based on the associated pipeline's output. If the link element does not have the `data-trunk` attribute, it will not be processed.

### fixed
- Fixed [#50](https://github.com/thedodd/trunk/issues/50): the ability to copy a file or an entire dir into the dist dir is now supported with two different pipeline types: `<link data-trunk rel="copy-file" href="target/file"/>` and `<link data-trunk rel="copy-dir" href="target/dir"/>` respectively.

### removed
- The `manifest-path` option has been removed from all Trunk subcommands. The path to the `Cargo.toml` is now specified in the source HTML as `<link rel="rust" href="path/to/Cargo.toml"/>`. The `href="..."` attribute may be omitted, in which case Trunk will look for a `Cargo.toml` within the same directory as the source HTML. If the `href` attribute points to a directory, Trunk will look for the `Cargo.toml` file in that directory.

## 0.6.0
### added
- Closed [#59](https://github.com/thedodd/trunk/issues/55): Support for writing the public URL (`--public-url`) to the HTML output.

### fixed
- Closed [#62](https://github.com/thedodd/trunk/issues/62): Improved handling of file paths declared in the source `index.html` to avoid issues on Windows.
- Closed [#58](https://github.com/thedodd/trunk/issues/58): The output WASM file generated from the cargo build is now determined purely based on a JSON build plan provided from cargo itself. This will help to provide a more stable pattern for finding build artifacts. If you were running into issues where Trunk was not able to find the WASM file built from cargo due to hyphens or underscores in the name, that problem should now be a thing of the past.
- The default location of the `dist` dir has been slightly modified. The `dist` dir will now default to being generated in the parent dir of cargo's `target` dir. This helps to make behavior a bit more consistent when executing trunk for locations other than the CWD.
- Fixed an issue where paths declared in a `Trunk.toml` file were not being treated as relative to the file itself.

## 0.5.1
### fixed
- Closes [#55](https://github.com/thedodd/trunk/issues/55): Fixed a regression in the server where the middleware was declared after the handler, and was thus not working as needed. Putting the middleware first fixes the issue.

## 0.5.0
### added
- Added support for proxying requests to arbitrary HTTP backends.
    - This includes a basic CLI based config.
    - This also includes a more robust `Trunk.toml` config which allows for specifying multiple proxies.
- Added a `trunk config show` subcommand. This command will pretty-print Trunk's final runtime config based on any config file & env vars. This can be useful for debugging & testing.

## 0.4.0
### added
- In addition to CLI arguments and options, Trunk now supports layered configuration via `Trunk.toml` & environment variables.
- Added an example `Trunk.toml` to the root of the repository showing all possible config values along with their defaults.

### changed
- README has been updated with details on how the config system works.
- Removed a fair amount of code duplication as part of the configuration changes.
- Added full release automation with optimized release binaries for Linux, MacOS & Windows (all x64). Brew packages for MacOS and Linux, and a Chocolatey package for Windows coming soon.

### fixed
- Closed [#37](https://github.com/thedodd/trunk/issues/37): Trunk now exits with a non-zero code when an error takes place during execution.
- Closed [#40](https://github.com/thedodd/trunk/issues/40): Trunk is now copying JS snippets from wasm-bindgen into the dist dir as part of the standard build/watch/serve commands.

## 0.3.1
### fixed
- Fixed a regression in resolving `cargo build`'s output WASM.

## 0.3.0
### added
- Handle multi-project & workspace contexts. Thank you @oli-obk for developing [cargo_metadata](https://github.com/oli-obk/cargo_metadata).

## 0.2.0
### changed
- All CLI output has been improved using console & indicatif. Builds, asset pipelines, and the like are using a progress spinner along with messages. All in all this provides a much more clean CLI output experience.
- Using `console::Emoji` to ensure that emojis are only sent to stdout/stderr when supported.
- All builds performed by trunk now emit warnings when a link is found in the HTML which is an invalid FS path. This does not effect hyperlinks of course.
- `trunk serve` now takes the `--open` option to control whether or not a browser tab will be opened. Thanks @MartinKavik for the report.
- The `trunk serve` listener info has been slightly updated. It continues to function as in previous releases. Thanks @MartinKavik for the report.

## 0.1.3
### fixed
- Closed [#15](https://github.com/thedodd/trunk/issues/15): ensure cargo package name is processed as cargo itself processes package names (`s/-/_/`).
- Closed [#16](https://github.com/thedodd/trunk/issues/16): default to `index.html` as the default target for all CLI commands which expect a target. This matches the expectation of Seed & Yew.

Thanks @MartinKavik for reporting these items.

## 0.1.2
### changed
- Swap our `grass` for `sass-rs`.
- Compress SASS/SCSS output when building in `--release` mode.

## 0.1.1
### fixed
- Fix an issue with the watch system which was breaking builds on Windows.

## 0.1.0
- Initialize release. See the [release notes on Github](https://github.com/thedodd/trunk/releases/tag/v0.1.0) for more info.
