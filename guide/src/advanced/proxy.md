# Backend Proxy

Trunk ships with a built-in proxy which can be enabled when running `trunk serve`. There are two ways to configure the
proxy, each discussed below. All Trunk proxies will transparently pass along the request body, headers, and query
parameters to the proxy backend.

### Proxy CLI Flags

The `trunk serve` command accepts two proxy related flags.

`--proxy-backend` specifies the URL of the backend server to which requests should be proxied. The URI segment of the
given URL will be used as the path on the Trunk server to handle proxy requests.
E.G., `trunk serve --proxy-backend=http://localhost:9000/api/` will proxy any requests received on the path `/api/` to
the server listening at `http://localhost:9000/api/`. Further path segments or query parameters will be seamlessly
passed along.

`--proxy-rewrite` specifies an alternative URI on which the Trunk server is to listen for proxy requests. Any requests
received on the given URI will be rewritten to match the URI of the proxy backend, effectively stripping the rewrite
prefix. E.G., `trunk serve --proxy-backend=http://localhost:9000/ --proxy-rewrite=/api/` will proxy any requests
received on `/api/` over to `http://localhost:9000/` with the `/api/` prefix stripped from the request, while everything
following the `/api/` prefix will be left unchanged.

`--proxy-insecure` allows the `--proxy-backend` url to use a self-signed certificate for https (or any
officially [invalid](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.danger_accept_invalid_certs)
certs, including expired). This would be used when proxying to https such
as `trunk serve --proxy-backend=https://localhost:3001/ --proxy-insecure` where the ssl cert was self-signed, such as
with [mkcert](https://github.com/FiloSottile/mkcert), and routed through an https reverse proxy for the backend, such
as [local-ssl-proxy](https://github.com/cameronhunter/local-ssl-proxy)
or [caddy](https://caddyserver.com/docs/quick-starts/reverse-proxy).

`--proxy-no-sytem-proxy` bypasses the system proxy when contacting the proxy backend.

`--proxy-ws` specifies that the proxy is for a WebSocket endpoint.

### Config File

The `Trunk.toml` config file accepts multiple `[[proxy]]` sections, which allows for multiple proxies to be configured.
Each section requires at least the `backend` field, and optionally accepts the `rewrite` and `ws` fields, both
corresponding to the `--proxy-*` CLI flags discussed above.

As it is with other Trunk config, a proxy declared via CLI will take final precedence and will cause any config file
proxies to be ignored, even if there are multiple proxies declared in the config file.

The following is a snippet from the `Trunk.toml` file in the Trunk repo:

```toml
[[proxy]]
rewrite = "/api/v1/"
backend = "http://localhost:9000/"
```
