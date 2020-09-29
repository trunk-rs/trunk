changelog
=========
This changelog follows the patterns described here: https://keepachangelog.com/en/1.0.0/.

## Unreleased

### added
- Support for writing the public URL (`--public-url`) to the HTML output. ([#59](https://github.com/thedodd/trunk/issues/55))

### fixed

- Closes [#62](https://github.com/thedodd/trunk/issues/62): Fixed an issue where the path of an asset file was mapped to an invalid path on Windows systems.

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
