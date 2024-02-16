# Trunk behind a reverse proxy

The idea is to have `trunk serve` running behind a reverse proxy. This may be suitable for testing,
but `trunk serve` is not intended to host your application.

**NOTE:** This is different to using trunk's built in proxy, which by itself acts as a reverse proxy, allowing to
access other endpoints using the endpoint served by `trunk serve`. 

**NOTE**: All commands are relative to this file.

## Running the example

As reverse proxy, we run an NGINX:

```shell
podman run --rm --network=host -ti  -v $(pwd)/nginx.conf:/etc/nginx/nginx.conf:z docker.io/library/nginx:latest
```

And then we serve the "vanilla" example using trunk:

```shell
trunk serve ../vanilla/Trunk.toml --public-url /my-app --serve-base /
```

Or, using the development version:

```shell
cargo run --manifest-path=../../Cargo.toml -- serve --config ../vanilla/Trunk.toml --public-url /my-app --serve-base /
```

## Trying it out

When you go to <http://localhost:9090>, you will see the NGINX welcome screen.

When you go to <http://localhost:9090/my-app/>, you will see the application served by `trunk`.
