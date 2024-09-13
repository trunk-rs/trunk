# Hooks

If you find that you need Trunk to perform an additional build action that isn't supported directly, then Trunk's
flexible hooks system can be used to launch external processes at various stages in the pipeline.

## Build steps

This is a brief overview of Trunk's build process for the purpose of describing when hooks are executed. Please note
that the exact ordering may change in the future to add new features.

- Step 1 — Read and parse the HTML file.
- Step 2 — Produce a plan of all assets to be built.
- Step 3 — Build all assets in parallel.
- Step 4 — Finalize and write assets to staging directory.
- Step 5 — Write HTML to staging directory.
- Step 6 - Replace `dist` directory contents with staging directory contents.

The hook stages correspond to this as follows:

- `pre_build`: takes place before step 1.
- `build`: takes place at the same time as step 3, executing in parallel with asset builds.
- `post_build`: takes place after step 5 and before step 6.

## Hook execution

Hooks can be declared exclusively in `Trunk.toml`, and consist of a `stage`, `command` and `command_arguments`:

- `stage`: (required) one of `pre_build`, `build` or `post_build`. It specifies when in Trunk's build pipeline the hook
  is executed.
- `command`: (required) the name or path to the desired executable.
- `command_arguments`: (optional, defaults to none) any arguments to be passed, in the given order, to the executable.

At the relevant point for each stage, all hooks for that stage are spawned simultaneously. After this, Trunk immediately
waits for all the hooks to exit before proceeding, except in the case of the `build` stage, described further below.

All hooks are executed using the same `stdin` and `stdout` as trunk. The executable is expected to return an error code
of `0` to indicate success. Any other code will be treated as an error and terminate the build process. Additionally,
the following environment variables are provided to the process:

- `TRUNK_PROFILE`: the build profile in use. Currently, either `debug` or `release`.
- `TRUNK_HTML_FILE`: the full path to the HTML file (typically `index.html` in `TRUNK_SOURCE_DIR`) used by trunk.
- `TRUNK_SOURCE_DIR`: the full path to the source directory in use by Trunk. This is always the directory in
  which `TRUNK_HTML_FILE` resides.
- `TRUNK_STAGING_DIR`: the full path of the Trunk staging directory.
- `TRUNK_DIST_DIR`: the full path of the Trunk dist directory.
- `TRUNK_PUBLIC_URL`: the configured public URL for Trunk.

## OS-specific overrides

Often times you will want to perform the same build step on different OSes, requiring different commands. 
A typical example of this is using the `sh` command on Linux, but `cmd` on Windows. 
To accomodate this, you can optionally create OS-specific overrides for each hook. 
To do this, specify the default hook, then directly below it create a `[hooks.<os>]` entry where `<os>` 
can be one of `windows`, `macos`, or `linux`. Within this entry you must specify only the `command` and 
`command_argumnets` keys. You may provide multiple overrides for each hook. i.e. 
One for `windows`, one for `macos`, and one for `linux`.

