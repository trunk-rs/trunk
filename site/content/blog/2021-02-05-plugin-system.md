+++
title = "Design Thoughts — Trunk Plugin System"
description = "Thoughts and considerations on various design patterns for a Trunk plugin system."
[extra]
author = "Doddzilla"
+++

Hello again! For this post, I would like to share my thoughts on a few possible design patterns for a Trunk plugin system. We will review a subset of the possible patterns based on some community discussion so far, looking at pros, cons, limitations and other things to consider along the way.

# Why Plugins?
The Trunk community has been discussing plans to introduce a plugin system for Trunk. A good question to ask is: why? There are definitely a few good reasons:

- There is a lot more work to be done on Trunk, and plugins would allow for folks to quickly and easily integrate with the Trunk build system in ways which are not yet supported.
- Proprietary integration. Perhaps folks have some Trunk pipelines they would like to build, but they are not able to open source the code. This would provide for such cases.
- Perhaps folks don't want to take the time to contribute 😬. The maintainers are too mean, too snarky, they always ask me to change things ... who knows?
- Perhaps the use case is too niche, and the Trunk maintainers don't think it should be in the code base.
- A big one is maintenance. As Trunk grows and new use cases inevitibly emerge, that will be more and more code to maintain, which may slow overall progress of core Trunk development.

# Trunk Internals (abridged)
To have a meaningful discussion on possible Trunk plugin designs, let's take a quick look at the internals of Trunk. There are 3 primary layers:

- **Configuration:** this includes `Trunk.toml`, env vars, & CLI args/opts. These are merged based on a cascade and then fed into the lower layers.
- **CLI Commands:** this is what tells Trunk what to do.
- **Pipelines:** all of the goodness of Trunk comes from the various pipelines we've created in this layer. This is where we process HTML, SASS/SCSS, CSS, images, JS, etc.

## Pipelines
The entire build system is a composition of various asynchronous pipelines. The very first pipeline is the HTML pipeline. This directly corresponds to the source HTML file (typically the `index.html`), and this pipeline is responsible for spawning various other pipelines based on the directives found in the source HTML. As a reminder, Trunk uses the `<link data-trunk rel="{{TYPE}}" ..args.. />` pattern in the source HTML to declare asset pipelines which Trunk should process.

Each pipeline is given all of the data found in its corresponding `<link>`, along with some additional general data, like config and such. This is then spawned off as an async task. Currently there are no dependencies between pipeliens, however the HTML pipeline will not complete until all of its spawned pipelines have finished.

Ultimately, each pipeline can do whatever it needs to do, producing various artifacts and writing them to the Trunk staging directory. Once a pipeline finishes, it will return a manifest of data along with a callback. That callback is called by Trunk providing a mutable reference to the will-be final HTML, and that callback can update the HTML in whatever way it needs, which is typically just to remove its original `link`, and maybe append some new data which points to newly generated artifacts. Once all of the pipelines have completed, Trunk will perform a few final tasks to provide a nice pristine `dist` dir ready to be served to the web.

Simply speaking, it could be said that Trunk is really just a system for running various sorts of pipelines/tasks which operate on an HTML document.

# Embedded WASM Runtime
One pattern which is rather exciting would be to use an embedded WASM runtime like [https://wasmer.io/](https://wasmer.io/) or [https://wasmtime.dev/](https://wasmtime.dev/) as the basis for our plugin system.

We could define a WASM import/export interface where a WASM module may be loaded by Trunk, we query its Trunk ABI version, and then we call that module's `main` function (or whatever we decide to call it) and pass it a compatible data structure corresponding to the data which normal Trunk pipelines would receive.

Those WASM plugin functions could return compatible data structures and manifests just like normal pipelines, and then we call another function on that WASM module, say `callback`, providing it HTML and then expecting HTML in response. This would be pretty damn close to the current pipeline system.

## Plugin Discovery
Plugins could be declared in the `Trunk.toml`. A section of the TOML, say `[plugins]`, would allow users to declare a module by name, along with some info on how to download the corresponding WASM. Trunk would fetch and cache these modules, and then modules could be selected for use in the source HTML via a `<link data-trunk rel="plugin" data-plugin="{{NAME}}" .../>` or the like. This would give Trunk a fairly robust mechanism for detecting that a directive should use a plugin, and we would also know the name of the plugin as provided by the `data-plugin` attr.

I've considered perhaps leveraging [https://wapm.io/](https://wapm.io/) for this. Folks could use the WAPM registry for their Trunk plugins. Maybe we just allow folks to specify a URL (like the URL of an asset from a Github release) where Trunk can download the WASM module. Perhaps we support multiple patterns.

## Capability Attrs
When a plugin is declared in the source HTML, attributes could be provided in the corresponding link which declare the "capabilities" which should be given to the plugin. Something like `data-plugin-capabilities="capX,capY"`, or perhaps something more granular. Various capabilities could be listed, giving the Trunk user full control over what the plugin can and can not do.

Having used many JS bundlers and build tools, many of which have their own plugin systems, I have very often been quite paranoid about not knowing EXACTLY what those plugins are doing. Lots of fun exploits out there. This seems like a great way to help mitigate such issues. Plugins could declare the capabilities they need, and Trunk can abort a pipeline early if the user has not provided the needed capabilities, helping to ensure users don't run into unexpected issues.

This seems quite nice. There are lots of additional features which could be developed along these lines. New Trunk capabilities could be exposed over time as we develop new features. I dig it.

## Trunk Plugin Lib
Along with this approach we would release a `trunk-plugin` crate which does everything for you. The only thing plugin authors would need to do is write the actual business logic of their plugin. Some of the things it would do:

- it would have cargo features for opting into the various capabilities which Trunk can expose to plugins, and would expose code to actually utilize those capabilities from within the plugin.
- opting into various capabilities would automatically update an exported WASM function to tell Trunk which capabilities the plugin needs.
- all of the WIT business would be taken care of by this crate. Authors would not have to worry about how to move data across the WASM boundary.

## Pros
Ok, so let's recap the goodness.

- WASM gives us a universal runtime, so plugins could be written in any compatible WASM language (Rust, C, C++, Swift, Go, AssemblyScript, &c).
- Plugins would only ever need to be compiled for one architecture: WASM.
- Capability-based security. If a plugin needs network or filesystem access, this might be a good time to go review the code in the plugin.
- The Trunk maintainers would publish a crate to handle all of the heavy lifting related to creating plugins.

## Cons
- Building capabilities will take time and deeper design work, though this will be true for any plugin system.
- WASM does not yet support async paradigms, so basically all plugins will need to be spawned onto an async threadpool. Not really a big deal.
- We would be moving somewhat out of the happy and safe Rustacean tide pool and into the cold scary world of WASM, other package managers (potentially), and an unstable WASM/WASI/WIT set of specs ... though we are already a Rust WASM web bundler. Additionally, we would quite likely not use WASI at all, and instead expose our own set of capabilities.

# Dynamic Linking (via Stable ABI)
We've looked into using [abi_stable_crates](https://github.com/rodrimati1992/abi_stable_crates/). This would be Rust dynamic linking via a stable subset of the Rust ABI (provided by this crate, because Rust itself does not have a stable ABI), and then leveraging this crate's runtime logic to verify the safety of dynamically linking to the various plugins.

We would need our own discovery system for this, and plugins would need to be built by Trunk (most likely), as they would need to be built for the host tripple. There would be a fair bit of overhead with this approach. It would not be as rigid as C FFI, and it would still be safe Rust<->Rust communication, but we would need to use a limited subset of types at the boundary.

# Just Don't
We could just not do a plugin system. There are already a few nice improvements we can make to the current process of adding new pipeline types. By my analysis, we should switch over to a dynamic dispatch model for the response data from pipelines. I only say this because the current approach uses enums (which I love), but folks unfamiliar with the code base may see this as tedious to update.

The process of adding new pipeline types is already pretty simple, we can make it even more simple. Perhaps this topic merits a blog post of its own.

# Conclusion
All in all, I'm pretty stoked about the possibility of using a WASM runtime for Trunk plugins. The security improvements (over other plugin models) are real, the simplicity of distributing ready to execute WASM is real, I dig it.

There is some good discussion over in [Trunk #104](https://github.com/thedodd/trunk/issues/104), and more discussion to be had. This blog post should be seen as part of that discussion. If you have thoughts or ideas on this topic, please drop by and share. We would love to hear from you!

Cheers!
