<h1 align="center">trunk</h1>
<div align="center">
  <strong>
    Build, bundle & ship your Rust WASM application to the web.
  </strong>
  <br/>
  <i>
    ”Pack your things, we’re going on an adventure!” ~ Ferris
  </i>
</div>
<br/>

Goals:
- Leverage the wasm-bindgen ecosystem for all things related to building Rust WASM for the web.
- Simple, zero-config bundling of WASM, JS bridge & other assets (images, css, scss) via a source HTML file.
- Rust WASM first. Not the other way around.

## commands
### build
- future: make this all based on a target html file. For now, just make it 100% based on the wasm-bindgen flow.
  - needs to take a source html file and output an updated html file which reference the hashed values.
  - let's use https://github.com/causal-agent/scraper to parse, manipulated, and output the html.
- invoke cargo build based on defaults, ensure wasm32 is the target.
- invoke wasm-bindgen to produce the output wasm and the JS bridge.
  - build on wasm-bindgen & ″-cli. Maybe in the future, we can incorproate some of that functionality into the CLI.
  - send output to target/wasm-bindgen/<mode>/
- take wasm-bindgen output. Generate a hash sha1 over the wasm & js. Copy the wasm & js to a ./dist/ dir with hashes appended to the end of the filename before extension.
- sass or scss referenced in the HTML will be compiled by https://github.com/connorskees/grass, and then we'll write it out to a css file bearing the same name, with a hash suffixed to the name.
  - long-term, we can look into a convention where we descend over the source tree and find any sass/scss/css and add them all to a single output file which will be injected into the output HTML.

### watch
- same as build, except will use https://github.com/notify-rs/notify to trigger new build invocations.

### serve
- same as watch, except will also serve content on localhost:8080.

### ship
- should do everything that build does, except in release mode, perhaps with some size optimizations and such.
