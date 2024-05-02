# Base URLs, public URLs, paths & reverse proxies

Since: `0.19.0-alpha.3`.

Originally `trunk` had a single `--public-url`, which allowed to set the base URL of the hosted application.
Plain and simple. This was a prefix for all URLs generated and acted as a base for `trunk serve`.

Unfortunately, life isn't that simple and naming is hard.

Today `trunk` was three paths:

* The "public base URL": acting as a prefix for all generated URLs
* The "serve base": acting as a scope/prefix for all things served by `trunk serve`
* The "websocket base": acting as a base path for the auto-reload websocket

All three can be configured, but there are reasonable defaults in place. By default, the serve base and websocket base
default to the absolute path of the public base. The public base will have a slash appended if it doesn't have one. The
public base can be one of:

* Unset/nothing/default (meaning `/`)
* An absolute URL (e.g. `http://domain/path/app`)
* An absolute path (e.g. `/path/app`)
* A relative path (e.g. `foo` or `./`)

If the public base is an absolute URL, then the path of that URL will be used as serve and websocket base. If the public
base is a relative path, then it will be turned into an absolute one. Both approaches might result in a dysfunctional
application, based on your environment. There will be a warning on the console. However, by providing an explicit
value using serve-base or ws-base, this can be fixed.

Why is this necessary and when is it useful? It's mostly there to provide all the knobs/configurations for the case
that weren't considered. The magic of public-url worked for many, but not for all. To support such cases, it
is now possible to tweak all the settings, at the cost of more complexity. Having reasonable defaults should keep it
simple for the simple cases.

An example use case is a reverse proxy *in front* of `trunk serve`, which can't be configured to serve the trunk
websocket at the location `trunk serve` expects it. Now, it is possible to have `--public-url` to choose the base when
generating links, so that it looks correct when being served by the proxy. But also use `--serve-base /` to keep
serving resource from the root.
