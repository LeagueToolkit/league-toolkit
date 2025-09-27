# league-toolkit

Rust workspace for parsing, editing, and writing League of Legends file formats.

This repository hosts a set of crates that can be used individually or together via the umbrella crate `league-toolkit`.

## Crates in this workspace

- `league-toolkit` — Library for serializing and editing various League of Legends formats (feature-gated facade over the crates below)
- `ltk_anim` — Animation formats support for League Toolkit
- `ltk_file` — Core IO and file abstractions for League Toolkit
- `ltk_mesh` — Mesh parsing and structures for League Toolkit
- `ltk_meta` — Metadata formats and utilities for League Toolkit
- `ltk_primitives` — Primitive types and helpers for League Toolkit
- `ltk_texture` — Texture decoding/encoding utilities for League Toolkit
- `ltk_wad` — WAD archive reading/writing for League Toolkit
- `ltk_hash` — Hashes implementation used by League Toolkit
- `ltk_io_ext` — I/O extensions used by League Toolkit

Each crate lives under `crates/<name>`.

## Getting started

Add the umbrella crate to your project (recommended):

```toml
# Cargo.toml
[dependencies]
league-toolkit = { version = "0.1", features = ["wad", "mesh", "texture"] }
```

Until a release is published, you can use the Git version:

```toml
[dependencies]
league-toolkit = { git = "https://github.com/LeagueToolkit/league-toolkit", features = ["wad", "mesh", "texture"] }
```

Or depend on individual crates directly, for example:

```toml
[dependencies]
ltk_wad = "0.1"
ltk_texture = "0.1"
```

## Features (on `league-toolkit`)

The `league-toolkit` crate exposes feature flags to opt into specific subsystems:

- `anim` — enable `ltk_anim`
- `file` — enable `ltk_file`
- `mesh` — enable `ltk_mesh`
- `meta` — enable `ltk_meta`
- `primitives` — enable `ltk_primitives`
- `texture` — enable `ltk_texture`
- `wad` — enable `ltk_wad`
- `hash` — enable `ltk_hash`
- `serde` — enable serde support where available

The default feature set enables most subsystems. Disable default features and opt-in selectively if you want a smaller dependency surface:

```toml
[dependencies]
league-toolkit = { version = "0.1", default-features = false, features = ["wad", "mesh"] }
```

## Development

- Prerequisites: Rust stable toolchain
- Build: `cargo build`
- Test: `cargo test`

Workspace membership is defined in the top-level `Cargo.toml`.

## Releasing

This repository uses Release-plz to automate versioning and publishing to crates.io.

- On pushes to `main`, Release-plz opens a release PR.
- Merging the release PR triggers publishing of the configured crates.

Make sure the repository has the appropriate credentials configured (crates.io token or Trusted Publishing) before merging release PRs.

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0-only).

See the `LICENSE` file for details.
