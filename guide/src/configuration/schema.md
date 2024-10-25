# Configuration Schema

Trunk provides a JSON schema for the configuration model. This can be added to e.g. a YAML file using the following
syntax:

```yaml
$schema: "./schema.json"
```

## Obtaining the schema

You can generate the schema by running:

```bash
trunk config generate-schema
```

Or directly write it to a file:

```bash
trunk config generate-schema path/to/file
```

## Editor/IDE support

Your editor/IDE needs to support this functionality. Trunk only provides the schema. The following sections provide
some information on how to use this.

### IntelliJ (and alike)

IntelliJ based IDEs (including Rust Rover) do support JSON schemas in YAML and JSON files. You only need to reference
the schema, like:

```yaml
$schema: "./schema.json"
```
