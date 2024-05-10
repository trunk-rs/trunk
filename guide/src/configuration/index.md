# Configuration

```admonish important
Trunk's configuration has massively changed in the 0.21.0 release. The goal was not to break anything, but it might
have happened anyway. Also does the layering system work a bit different now.

It might also be that the documentation still mentions only `Trunk.toml`. If that's the case, then this now includes
all other configuration file variants as well.
```

Trunk supports a layered configuration system. The base comes from a reasonable set of defaults, overridden by
a configuration file, overridden command line arguments.

Technically speaking, there's a project configuration struct, which has reasonable defaults. Trunk will try to locate
a configuration file and load if into this struct. It will then override this configuration with settings from the
command line parser (which includes environment variables).

## Configuration files

Trunk will try to locate a configuration file. Either in the local directory, or by using the global argument
`--config`, which can accept either a file, or a directory. If the argument is a file, then this file will be
used directly. Otherwise, Trunk will load the first file found, searching for:

* `Trunk.toml`
* `.trunk.toml`
* `Trunk.yaml`
* `.trunk.yaml`
* `Trunk.json`
* `.trunk.json`

If neither of those files is found, Trunk will use the metadata from the `Cargo.toml`, which defaults to an empty
set of metadata.

The directory of the configuration file will become the project root, and all relative files will be resolved based
on that project root.

## Formats

Trunk's configuration is limited to a JSON compatible model. This means you can easily translate between those
different formats.

For example, having the following `Trunk.toml` configuration:

```toml
[build]
dist = "dist"
[serve]
port = 8080
```

Would be the following in YAML:

```yaml
build:
  dist: "dist"
serve:
  port: 8080
```

Also `Cargo.toml` is based on that model. However, it moves that data down into the `package.metadata.trunk` section.
The example above would become:

```toml
[package.metadata.trunk.build]
dist = "dist"
[package.metadata.trunk.serve]
port = 8080
```

## Command line arguments (and environment variables)

Command line arguments can override part of the configuration. Not all configuration aspects can be overridden by
the command line arguments though. Command line arguments include the use of environment variables.

Trunk supports `--help` on all levels of commands and sub-commands. This will show you the available options, as well
as the names of the environment variables to use instead.

All relative paths will be resolved against the project root, as evaluated by loading the configuration.

## Migration from pre 0.21.0 the best approach to moving forward

While the goal was to support all fields from `Trunk.toml`, the command line arguments as well as the environment
variables, it still is a version breaking the API. In some cases, it just made little sense, and so those fields
got marked "deprecated". They trigger a warning today and might be removed in one of the next releases.

Ideally, you don't need to change anything. In some ideal cases, you don't even need any configuration. In case you do,
you now have some more choices. You can keep using TOML, you may hide it using `.trunk.*` variant. You can use YAML or
JSON to leverage the JSON schema that is generated. Or if you're a fan of keeping everything in `Cargo.toml`, that's
fine too. The choice is yours.

```admonish important
You need to take care when working with older versions of Trunk though. If you use an older version of Trunk
(before 0.21.0) with a project using the newer configuration files, then that version would not consider those files
and might consider default settings, due to the missing `Trunk.toml` file.
```

## Required version

Starting with `0.19.0-alpha.2`, it is possible to enforce having a certain version of trunk building the project.

As new features get added to trunk, this might be helpful to ensure that the version of trunk building the current
is actually capable of doing so. This can be done using the `trunk-version` (or using the alias `trunk_version`) on
the **root** level of the `Trunk.toml` file.

The version format is a "version requirement", the same format you might know from Cargo's version field on
dependencies.

This also supports pre-release requirements, which allows adopting upcoming features early.

```admonish note
Versions prior do `0.19.0-alpha.2` currently do not support this check, and so they will silently ignore
such an error for now.
```
