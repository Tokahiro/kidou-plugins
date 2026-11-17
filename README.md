# Kidō plugins

The official plugin list for Kidō.

- `index.toml` lists each plugin: id, name, version, a short description, the package address and the package's SHA-256.
- `packages/` holds the `.kidouplug` files. A package never changes once listed; a new version gets a new file.

Kidō refuses a package whose SHA-256 differs from the one in `index.toml`.
