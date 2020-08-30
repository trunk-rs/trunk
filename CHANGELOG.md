changelog
=========

## Unreleased

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
