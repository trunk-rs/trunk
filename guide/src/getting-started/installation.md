# Installation

`trunk` is a standard Rust command line tool and can be installed using standard Rust tooling (`cargo`), by downloading
a pre-compiled binary, or through some distribution packagers.

## Installing from source

As `trunk` uses a standard Rust build and release process, you can install `trunk` just the "standard way". The
following sections will give some examples.

`trunk` supports a build time features, they are:

<dl>
<dt><code>rustls</code> (default)</dt><dd>Use rustls for client and server sockets</dd>
<dt><code>native-tls</code></dt><dd>Enable the use of the system native TLS stack for client sockets, and `openssl` for server sockets</dd>
<dt><code>update_check</code> (default)</dt><dd>Enable the update check on startup</dd>
</dl>

### Installing a release from crates.io

As `trunk` is released on [crates.io](https://crates.io/crates/trunk), it can be installed by simply executing:

```shell
cargo install --locked trunk
```

### Installing from git directly

Using `cargo` you can also install directly from git:

```shell
cargo install --git https://github.com/trunk-rs/trunk trunk
```

This will build and install the most recent commit from the `main` branch. You can also select a specific commit:

```shell
cargo install --git https://github.com/trunk-rs/trunk trunk --rev <commit>
```

Or a specific tag:

```shell
cargo install --git https://github.com/trunk-rs/trunk trunk --tag <tag>
```

### Installing from the local directory

Assuming you have checked out the `trunk` repository, even with local changes, you can install a local build using: 

```shell
cargo install --path . trunk
```

## Installing a pre-compiled binary from `trunk`

Pre-compiled releases have the `default` features enabled.

### Download from GitHub releases

`trunk` published compiled binaries for various platforms during the release process. They can be found in the
[GitHub release section](https://github.com/trunk-rs/trunk/releases) of `trunk`. Just download and extract the binary
as you would normally do.

### Using `cargo binstall`

[`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) allows to install pre-compiled binaries in a
more convenient way. Given a certain pattern, it can detect the version from crates.io and then fetch the matching
binary from a GitHub release. `trunk` supports this pattern. So assuming you have installed `cargo-binstall` already,
you can simpy run:

```shell
cargo binstall trunk
```

## Distributions

Trunk is released by different distributions. In most cases, a distribution will build their own binaries and might
not keep default feature flags. It might also be that an update to the most recent version might be delayed by the
distribution's release process.

As distributions will have their own update management, most likely Trunk's update check is disabled.

### Brew

`trunk` is available using `brew` and can be installed using:

```shell
brew install trunk
```

### Fedora

Starting with Fedora 40, `trunk` can be installed by executing:

```shell
sudo dnf install trunk
```

### Nix OS

Using Nix, `trunk` can be installed using:

```shell
nix-env -i trunk
```

## Update check

Since: `0.19.0-alpha.2`.

Trunk has an update check built in. By default, it will check the `trunk` crate on `crates.io` for a newer
(non-pre-release) version. If one is found, the information will be shown in the command line.

This check can be disabled entirely, by not enabling the cargo feature `update_check`. It can also be disabled during
runtime using the environment variable `TRUNK_SKIP_VERSION_CHECK`, or using the command line switch
`--skip-version-check`.

The actual check with `crates.io` is only performed every 24 hours.
