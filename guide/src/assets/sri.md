# Sub-resource integrity

Trunk can automatically generate hashes of files and add the `integrity` attribute for resources fetched by the web
application. This is enabled by default, but can be overridden using the `data-integrity` attribute. See the different
asset types.

The following values are available:

* `none`
* `sha256`
* `sha384` (default)
* `sha512`
