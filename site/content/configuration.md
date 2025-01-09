+++
title = "Configuration"
description = "Configuration"
weight = 2
+++

Trunk supports a layered config system. At the base, a config file can encapsulate project specific defaults, paths, ports and other config. Environment variables can be used to overwrite config file values. Lastly, CLI arguments / options take final precedence.

# Trunk.toml

Trunk supports an optional `Trunk.toml` config file. An example config file is included [in the Trunk repo](https://github.com/trunk-rs/trunk/blob/main/Trunk.toml), and shows all available config options along with their default values. By default, Trunk will look for a `Trunk.toml` config file in the current working directory. Trunk supports the global `--config` option to specify an alternative location for the file.

Note that any relative paths declared in a `Trunk.toml` file will be treated as being relative to the `Trunk.toml` file itself.

## Trunk Version

Starting with `0.19.0-alpha.2`, it is possible to enforce having a certain
version of trunk building the project.

As new features get added to trunk, this might be helpful to ensure that the
version of trunk building the current is actually capable of doing so. This can
be done using the `trunk-version` (or using the alias `trunk_version`) on the
**root** level of the `Trunk.toml` file.

The version format is a "version requirement", the same format you might know
from Cargo's version field on dependencies as a [Semantic Versioning
constraint](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#version-requirement-syntax).

This also supports pre-release requirements, which allows to adopt upcoming
features early.

**NOTE:** Versions prior do `0.19.0-alpha.2` currently do not support this
check, and so they will silently ignore such an error for now.

```toml
trunk-version = "^0.20.1"
```

## Build section

The build section has configuration settings for the build process. These
control the arguments passed to Cargo when building the application, and the
generation of the assets.

```toml
[build]
target = "index.html"       # The index HTML file to drive the bundling process.
html_output = "index.html"  # The name of the output HTML file.
release = false             # Build in release mode.
dist = "dist"               # The output dir for all final assets.
public_url = "/"            # The public URL from which assets are to be served.
filehash = true             # Whether to include hash values in the output file names.
inject_scripts = true       # Whether to inject scripts (and module preloads) into the finalized output.
offline = false             # Run without network access
frozen = false              # Require Cargo.lock and cache are up to date
locked = false              # Require Cargo.lock is up to date
minify = "never"            # Control minification: can be one of: never, on_release, always
no_sri = false              # Allow disabling sub-resource integrity (SRI)
```

## Watch section

Trunk has built-in support for watching for source file changes, which triggers
a rebuild and a refresh in the browser. In this section, you can override what
paths to watch and set files to be ignored.

```toml
[watch]
watch = []  # Paths to watch. The `build.target`'s parent folder is watched by default.
ignore = [] # Paths to ignore.
```

## Server section

Trunk has a built-in server for serving the application when running `trunk serve`.
This section lets you override how this works.

```toml
[serve]
addresses = ["127.0.0.1"]  # The address to serve on.
port = 8080                # The port to serve on.
aliases = ["http://localhost.mywebsite.com"] # The aliases to serve on.
open = false               # Open a browser tab once the initial build is complete.
no_spa = false             # Whether to disable fallback to index.html for missing files.
no_autoreload = false      # Disable auto-reload of the web app.
no_error_reporting = false # Disable error reporting
ws_protocol = "ws"         # Protocol used for autoreload WebSockets connection.
# Additional headers set for responses.
headers = { "test-header" = "header value", "test-header2" = "header value 2" }
# The certificate/private key pair to use for TLS, which is enabled if both are set.
tls_key_path = "self_signed_certs/key.pem"
tls_cert_path = "self_signed_certs/cert.pem"
```

## Clean section

The clean section controls the behaviour when running `trunk clean`, which will
remove build artifacts.

```toml
[clean]
dist = "dist" # The output dir for all final assets.
cargo = false # Optionally perform a cargo clean.
```

## Proxy section

The `Trunk.toml` config file accepts multiple `[[proxy]]` sections, which
allows for multiple proxies to be configured. Each section requires at least
the `backend` field, and optionally accepts the `rewrite` and `ws` fields, both
corresponding to the `--proxy-*` CLI flags discussed below.

As it is with other Trunk config, a proxy declared via CLI will take final
precedence and will cause any config file proxies to be ignored, even if there
are multiple proxies declared in the config file.

```toml
[[proxy]]
backend = "https://localhost:9000/api/v1" # Address to proxy requests to
ws = false                                # Use WebSocket for this proxy
insecure = false                          # Disable certificate validation
no_system_proxy = false                   # Disable system proxy
rewrite = ""                              # Strip the given prefix off paths
no_redirect = false                       # Disable following redirects of proxy responses
```

## Hooks section

Hooks are tasks that are run before, during or after the build. You can run
arbitrary commands here, and you can specify multiple hooks to run.

```toml
[[hooks]]
stage = "post_build"  # When to run hook, must be one of "pre_build", "build", "post_build"
command = "ls"        # Command to run
command_arguments = [] # Arguments to pass to command
```

# Environment Variables

Trunk environment variables mirror the `Trunk.toml` config schema. All Trunk environment variables have the following 3 part form `TRUNK_<SECTION>_<ITEM>`, where `TRUNK_` is the required prefix, `<SECTION>` is one of the `Trunk.toml` sections, and `<ITEM>` is a specific configuration item from the corresponding section. E.G., `TRUNK_SERVE_PORT=80` will cause `trunk serve` to listen on port `80`. The equivalent CLI invocation would be `trunk serve --port=80`.

In addition, there is the variable `TRUNK_SKIP_VERSION_CHECK` which allows to control the update check (if that is)
compiled into the version of trunk.

# CLI Arguments & Options

The final configuration layer is the CLI itself. Any arguments / options provided on the CLI will take final precedence over any other config layer.

# Proxy

Trunk ships with a built-in proxy which can be enabled when running `trunk serve`. There are two ways to configure the proxy, each discussed below. All Trunk proxies will transparently pass along the request body, headers, and query parameters to the proxy backend.

## Proxy CLI Flags

The `trunk serve` command accepts two proxy related flags.

`--proxy-backend` specifies the URL of the backend server to which requests should be proxied. The URI segment of the given URL will be used as the path on the Trunk server to handle proxy requests. E.G., `trunk serve --proxy-backend=http://localhost:9000/api/` will proxy any requests received on the path `/api/` to the server listening at `http://localhost:9000/api/`. Further path segments or query parameters will be seamlessly passed along.

`--proxy-rewrite` specifies an alternative URI on which the Trunk server is to listen for proxy requests. Any requests received on the given URI will be rewritten to match the URI of the proxy backend, effectively stripping the rewrite prefix. E.G., `trunk serve --proxy-backend=http://localhost:9000/ --proxy-rewrite=/api/` will proxy any requests received on `/api/` over to `http://localhost:9000/` with the `/api/` prefix stripped from the request, while everything following the `/api/` prefix will be left unchanged.

`--proxy-insecure` allows the `--proxy-backend` url to use a self signed certificate for https (or any officially [invalid](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.danger_accept_invalid_certs) certs, including expired). This would be used when proxying to https such as `trunk serve --proxy-backend=https://localhost:3001/ --proxy-insecure` where the ssl cert was self signed, such as with [mkcert](https://github.com/FiloSottile/mkcert), and routed through an https reverse proxy for the backend, such as [local-ssl-proxy](https://github.com/cameronhunter/local-ssl-proxy) or [caddy](https://caddyserver.com/docs/quick-starts/reverse-proxy).

`--proxy-no-sytem-proxy` bypasses the system proxy when contacting the proxy backend.

`--proxy-ws` specifies that the proxy is for a WebSocket endpoint.
