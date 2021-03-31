+++
title = "Configuration"
description = "Configuration"
weight = 2
+++

Trunk supports a layered config system. At the base, a config file can encapsulate project specific defaults, paths, ports and other config. Environment variables can be used to overwrite config file values. Lastly, CLI arguments / options take final precedence.

# Trunk.toml

Trunk supports an optional `Trunk.toml` config file. An example config file is included [in the Trunk repo](https://github.com/thedodd/trunk/blob/master/Trunk.toml), and shows all available config options along with their default values. By default, Trunk will look for a `Trunk.toml` config file in the current working directory. Trunk supports the global `--config` option to specify an alternative location for the file.

Note that any relative paths declared in a `Trunk.toml` file will be treated as being relative to the `Trunk.toml` file itself.

# Environment Variables

Trunk environment variables mirror the `Trunk.toml` config schema. All Trunk environment variables have the following 3 part form `TRUNK_<SECTION>_<ITEM>`, where `TRUNK_` is the required prefix, `<SECTION>` is one of the `Trunk.toml` sections, and `<ITEM>` is a specific configuration item from the corresponding section. E.G., `TRUNK_SERVE_PORT=80` will cause `trunk serve` to listen on port `80`. The equivalent CLI invocation would be `trunk serve --port=80`.

# CLI Arguments & Options

The final configuration layer is the CLI itself. Any arguments / options provided on the CLI will take final precedence over any other config layer.

# Proxy

Trunk ships with a built-in proxy which can be enabled when running `trunk serve`. There are two ways to configure the proxy, each discussed below. All Trunk proxies will transparently pass along the request body, headers, and query parameters to the proxy backend.

## Proxy CLI Flags

The `trunk serve` command accepts two proxy related flags.

`--proxy-backend` specifies the URL of the backend server to which requests should be proxied. The URI segment of the given URL will be used as the path on the Trunk server to handle proxy requests. E.G., `trunk serve --proxy-backend=http://localhost:9000/api/` will proxy any requests received on the path `/api/` to the server listening at `http://localhost:9000/api/`. Further path segments or query parameters will be seamlessly passed along.

`--proxy-rewrite` specifies an alternative URI on which the Trunk server is to listen for proxy requests. Any requests received on the given URI will be rewritten to match the URI of the proxy backend, effectively stripping the rewrite prefix. E.G., `trunk serve --proxy-backend=http://localhost:9000/ --proxy-rewrite=/api/` will proxy any requests received on `/api/` over to `http://localhost:9000/` with the `/api/` prefix stripped from the request, while everything following the `/api/` prefix will be left unchanged.

`--proxy-ws` specifies that the proxy is for a WebSocket endpoint.

## Config File

The `Trunk.toml` config file accepts multiple `[[proxy]]` sections, which allows for multiple proxies to be configured. Each section requires at least the `backend` field, and optionally accepts the `rewrite` and `ws` fields, both corresponding to the `--proxy-*` CLI flags discussed above.

As it is with other Trunk config, a proxy declared via CLI will take final precedence and will cause any config file proxies to be ignored, even if there are multiple proxies declared in the config file.

The following is a snippet from the `Trunk.toml` file in the Trunk repo:

```toml
[[proxy]]
rewrite = "/api/v1/"
backend = "http://localhost:9000/"
```
