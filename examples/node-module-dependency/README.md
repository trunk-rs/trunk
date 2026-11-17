# Node packages declared by a Rust dependency

The `patternfly-styles` crate declares the `@patternfly/patternfly` npm package, and the trunk
asset links it provides, in its `Cargo.toml`:

```toml
[[package.metadata.trunk.node_packages]]
name = "@patternfly/patternfly"
version = "6.4.0"
links = [
    { rel = "sass", href = "patternfly.scss" },
    { rel = "copy-dir", href = "assets", data-target-path = "assets/" },
]
```

The application only depends on `patternfly-styles`: no `Trunk.toml` entry and no `<link>` in
`index.html` are needed. The `href` of a link is relative to the installed package directory.
