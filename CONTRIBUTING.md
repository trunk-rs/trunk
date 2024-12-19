contributing
============
We are all living beings, and what is most important is that we respect each other and work together. If you can not uphold this simple standard, then your contributions are not welcome.

## hacking
Just a few simple items to keep in mind as you hack.

- Pull request early and often. This helps to let others know what you are working on. **Please use GitHub's Draft PR mechanism** if your PR is not yet ready for review.
- Use [conventional commits](https://www.conventionalcommits.org/en/v1.0.0/), a changelog will automatically be created from such commits
- When making changes to the configuration, be sure to regenate the schema. This can be done by running:

  ```shell
  cargo run -- config generate-schema schemas/config.json
  ```

## linting
We are using clippy & rustfmt. Clippy is SO GREAT! Rustfmt ... has a lot more growing to do; however, we are using it for uniformity.

Please be sure that you've configured your editor to use clippy & rustfmt, or execute them manually before submitting your code. CI will fail your PR if you do not.

## release workflow
We follow [semver](https://semver.org/spec/v2.0.0.html) for versioning this system.

- [ ] update `Cargo.toml` `version` & execute `cargo update` — this ensures that the `Cargo.lock` doesn't update during CI due to the new version number, which will cause CI failure.
- [ ] ensure CI completes successfully.
- [ ] add a new tag to the repo matching the new `Cargo.toml` `version`. Either via `git tag` or via the Github UI.
    - all release tags should start with the letter `v` followed by a semver version.
- [ ] CI is configured for release tags and will create a new GitHub release, and will upload release artifacts to the release page. Verify that this process has completed successfully.

## SSL

Trunk can use either `native-tls` or `rustls` for SSL support. `rustls` without `aws-lc-sys` is the default backend, which should build out-of-the-box on all platforms.

To opt into a different one, you can use the following command:
```sh
cargo build --no-default-features -F update_check,native-tls
```
If you want to use `native-tls` you can install OpenSSL using the instructions from one of the following resources:
+ https://stackoverflow.com/a/62729715/2961550
+ https://github.com/sfackler/rust-openssl/issues/1062#issuecomment-489441940
Or try using [rust-openssl from PR #2139](https://github.com/sfackler/rust-openssl/pull/2139) by updating the `Cargo.toml` file with
```diff
- openssl = { version = "0.10", default-features = false, optional = true }
+ openssl = { git = "https://github.com/micolous/rust-openssl", branch = "windows-build", default-features = false, optional = true }
```