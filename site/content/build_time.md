+++
title = "Build-time Dependency"
description = "Using Trunk as a build-time dependency"
weight = 4
+++

You can also use Trunk as a build-time dependency. This is useful if you want to build your application and then bundle inside another Rust application. The built application can be bundled using [rust-embed](https://github.com/pyrossh/rust-embed) or similar and then served by a webserver or any other way.

Here is an example:

**Cargo.toml**

```toml
# ...

[build-dependencies]
trunk = "0.18.0"

# ...
```

**build.rs**

```rust
use std::env;
use std::path::Path;
use trunk::{cmd::build::Build, config::ConfigOptsBuild};

#[tokio::main]
async fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=frontend");
    Build{
      build: ConfigOptsBuild {
        release: true,
        public_url: Some("/dist/".to_string()),
        target: Some(Path::new(FRONTEND).join("index.html").to_path_buf()),
        dist: Some(Path::new(env::var("OUT_DIR").unwrap().as_str()).join("frontend").to_path_buf()),
        ..ConfigOptsBuild::default()
      },
    }.run(None).await.unwrap();
}
```

**src/main.rs**

```rust
// ...
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$OUT_DIR/frontend"]
struct Asset;

fn main() {
    let index_html = Asset::get("index.html").unwrap();
    println!("{}", String::from_utf8(index_html.to_vec()).unwrap());
}
```
