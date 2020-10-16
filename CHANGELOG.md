changelog
=========
This changelog follows the patterns described here: https://keepachangelog.com/en/1.0.0/.

## Unreleased

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
