# Contributing

The repository holds the source of each plugin, the packages and the list of official plugins for Kidō (releasing soon):

```
index.toml       Kidō's list: each plugin's id, name, category, version, api, package and SHA-256
layouts/<id>/    the source of each Home layout
devices/<id>/    the source of each device plugin
sources/<id>/    the source of each source plugin
crates/layout/   the layout check, which Kidō builds too
wit/             the interfaces of the WebAssembly categories
packages/        every listed package; a file here never changes
xtask/           the check and the packer that CI runs
docs/            the guides for plugin writers
```

To change a plugin:

1. Change its source under `layouts/<id>/`, `devices/<id>/` or `sources/<id>/`.
2. Raise `version` in its `plugin.toml`. Leave `index.toml` and `packages/` alone.
3. Open a pull request.

CI checks that the list, the packages and the sources fit each other, and that the source packs.
For a plugin with code it first builds the component, `plugin.wasm`, for `wasm32-wasip2`.
The compiler, the dependencies and the release profile are fixed here, so the same source gives the same package.
A new compiler or dependency can change the component, and CI then asks for a new version of the plugin.
A component holds the paths of its source files as the build's system writes them, so compare a listed plugin with code on Linux, as CI does.
CI also runs the check Kidō runs on a Home layout at the install, from [`crates/layout/`](crates/layout), which is the code Kidō itself builds.
An SVG gets the time Kidō gives it there: 10 seconds, and 60 seconds for all of a layout's SVGs.
Kidō's check of the manifest isn't part of it, nor the memory limit Kidō sets for drawing an SVG on Windows, so install a new plugin with [`kidouplug`](docs/layouts.md#pack-and-try-it) once before you open the pull request.
After the merge it packs the new version, adds the file to `packages/` and updates the plugin's entry in `index.toml`.
Players get it the next time they open "Add plugins".

A new plugin also needs its `[[plugin]]` entry in `index.toml`, with its `id`, `name`, `category` and `about` only: CI adds `version`, `api`, `package` and `sha256`.
Kidō offers a listed plugin only where it runs the plugin's `api`.
Until it has that entry, CI checks and builds the source, and writes no package.

Run the same check yourself with a Rust toolchain. It builds Slint's compiler, which on Linux needs fontconfig's development files (`libfontconfig-dev`):

```
cargo xtask check
```

