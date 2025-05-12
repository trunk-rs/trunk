# Configuration

```admonish important
Trunk's configuration has massively changed in the 0.21.0 release. The goal was not to break anything, but it might
have happened anyway. Also does the layering system work a bit different now.

It might also be that the documentation still mentions only `Trunk.toml`. If that's the case, then this now includes
all other configuration file variants as well.
```

Trunk supports a layered configuration system. The base comes from a reasonable set of defaults, overridden by
a configuration file, overridden command line arguments.

Technically speaking, there's a project configuration struct, which has reasonable defaults. Trunk will try to locate
a configuration file and load if into this struct. It will then override this configuration with settings from the
command line parser (which includes environment variables).

## Configuration files

Trunk will try to locate a configuration file. Either in the local directory, or by using the global argument
`--config`, which can accept either a file, or a directory. If the argument is a file, then this file will be
used directly. Otherwise, Trunk will load the first file found, searching for:

* `Trunk.toml`
* `.trunk.toml`
* `Trunk.yaml`
* `.trunk.yaml`
* `Trunk.json`
* `.trunk.json`

If neither of those files is found, Trunk will use the metadata from the `Cargo.toml`, which defaults to an empty
set of metadata.

The directory of the configuration file will become the project root, and all relative files will be resolved based
on that project root.

## Formats

Trunk's configuration is limited to a JSON compatible model. This means you can easily translate between those
different formats.

For example, having the following `Trunk.toml` configuration:

```toml
[build]
dist = "dist"
[serve]
port = 8080
```

Would be the following in YAML:

```yaml
build:
  dist: "dist"
serve:
  port: 8080
```

Also `Cargo.toml` is based on that model. However, it moves that data down into the `package.metadata.trunk` section.
The example above would become:

```toml
[package.metadata.trunk.build]
dist = "dist"
[package.metadata.trunk.serve]
port = 8080
```

## Command line arguments (and environment variables)

Command line arguments can override part of the configuration. Not all configuration aspects can be overridden by
the command line arguments though. Command line arguments include the use of environment variables.

Trunk supports `--help` on all levels of commands and sub-commands. This will show you the available options, as well
as the names of the environment variables to use instead.

All relative paths will be resolved against the project root, as evaluated by loading the configuration.

## Migration from pre 0.21.0 the best approach to moving forward

While the goal was to support all fields from `Trunk.toml`, the command line arguments as well as the environment
variables, it still is a version breaking the API. In some cases, it just made little sense, and so those fields
got marked "deprecated". They trigger a warning today and might be removed in one of the next releases.

Ideally, you don't need to change anything. In some ideal cases, you don't even need any configuration. In case you do,
you now have some more choices. You can keep using TOML, you may hide it using `.trunk.*` variant. You can use YAML or
JSON to leverage the JSON schema that is generated. Or if you're a fan of keeping everything in `Cargo.toml`, that's
fine too. The choice is yours.

```admonish important
You need to take care when working with older versions of Trunk though. If you use an older version of Trunk
(before 0.21.0) with a project using the newer configuration files, then that version would not consider those files
and might consider default settings, due to the missing `Trunk.toml` file.
```

## Required version

Starting with `0.19.0-alpha.2`, it is possible to enforce having a certain version of trunk building the project.

As new features get added to trunk, this might be helpful to ensure that the version of trunk building the current
is actually capable of doing so. This can be done using the `trunk-version` (or using the alias `trunk_version`) on
the **root** level of the `Trunk.toml` file.

The version format is a "version requirement", the same format you might know from Cargo's version field on
dependencies.

This also supports pre-release requirements, which allows adopting upcoming features early.

```admonish note
Versions prior do `0.19.0-alpha.2` currently do not support this check, and so they will silently ignore
such an error for now.
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