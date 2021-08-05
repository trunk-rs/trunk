Trunk Proxy
===========
An example demonstrating how to use the Trunk proxy system for various HTTP endpoints as well as WebSockets.

There isn't much going on in this example as far as WASM is concerned, but have a look at the [`Trunk.toml`](./Trunk.toml) in this directory for exhaustive examples on how to configure and use Trunk proxies.

## Setup
First, in a shell terminal, execute `docker compose run --service-ports echo-server`. This will start an echo server container which we will use as a generic backend for the proxies we've defined in this example.

Next, in a different shell tab or window, execute `trunk serve`. This will build the demo application and will ultimately start the Trunk proxies defined in the [`Trunk.toml`](./Trunk.toml).

From here, use cURL, xh, websocat or any other tool you would like to verify the proxies' functionality. All normal HTTP requests will be echoed back, and all WebSocket messages will be echoed back.
