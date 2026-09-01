<div align="center">

# 🛠️ League Toolkit

**Rust library for parsing, editing, and writing League of Legends file formats**

[![CI](https://img.shields.io/github/actions/workflow/status/LeagueToolkit/league-toolkit/ci.yml?style=for-the-badge&logo=github)](https://github.com/LeagueToolkit/league-toolkit/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/league-toolkit.svg?style=for-the-badge&logo=rust)](https://crates.io/crates/league-toolkit)
[![Docs](https://img.shields.io/docsrs/league-toolkit?style=for-the-badge&logo=docs.rs)](https://docs.rs/league-toolkit)
[![License](https://img.shields.io/crates/l/league-toolkit.svg?style=for-the-badge)](https://github.com/LeagueToolkit/league-toolkit/blob/main/LICENSE)

[Documentation](https://docs.rs/league-toolkit) • [Crates.io](https://crates.io/crates/league-toolkit) • [Changelog](CHANGELOG.md)

</div>

---

## ✨ Features

- 📦 **WAD Archives** — Read and write `.wad.client` asset containers
- 🎨 **Textures** — Decode/encode `.tex` and `.dds` formats
- 🧍 **Meshes** — Parse skinned (`.skn`) and static (`.scb`/`.sco`) meshes
- 🦴 **Animation** — Load skeletons (`.skl`) and animations (`.anm`)
- 📋 **Property Bins** — Read/write `.bin` configuration files
- 🗺️ **Map Geometry** — Parse `.mapgeo` environment assets
- 🔧 **Modular** — Use individual crates or the umbrella crate

---

## 📦 Installation

Add the umbrella crate to your project:

```toml
[dependencies]
league-toolkit = { version = "0.2", features = ["wad", "mesh", "texture"] }
```

Or use individual crates for a smaller dependency footprint:

```toml
[dependencies]
ltk_wad = "0.2"
ltk_texture = "0.5"
ltk_mesh = "0.3"
```

---

## 🚀 Quick Start

### Reading a WAD Archive

```rust
use std::fs::File;
use ltk_wad::Wad;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("assets.wad.client")?;
    let mut wad = Wad::mount(file)?;
    
    println!("Archive contains {} files", wad.chunks().len());
    
    // Decode a specific chunk
    let (mut decoder, chunks) = wad.decode();
    for chunk in chunks.values().take(5) {
        let data = decoder.load_chunk_decompressed(chunk)?;
        println!("Chunk {:016x}: {} bytes", chunk.path_hash(), data.len());
    }
    
    Ok(())
}
```

### Decoding a Texture

```rust
use ltk_texture::Tex;
use std::fs::File;

let tex = Tex::from_reader(&mut File::open("texture.tex")?)?;
let surface = tex.decode_mipmap(0)?;
surface.into_rgba_image()?.save("output.png")?;
```

See the [`ltk_texture` README](crates/ltk_texture/README.md) for supported formats, raw pixel data access, and encoding.

### Parsing a Skinned Mesh

```rust
use ltk_mesh::SkinnedMesh;
use std::fs::File;

let mesh = SkinnedMesh::from_reader(&mut File::open("champion.skn")?)?;
println!("Vertices: {}", mesh.vertex_buffer().vertex_count());
println!("Submeshes: {}", mesh.ranges().len());
```

### Working with Property Bins

```rust
use ltk_meta::concrete::{values, Bin, BinObject};
use std::fs::File;

// Read
let bin = Bin::from_reader(&mut File::open("data.bin")?)?;
for (path_hash, object) in &bin.objects {
    println!("Object {path_hash:08x}");
}

// Create
let bin = Bin::builder()
    .dependency("shared/data.bin")
    .object(
        BinObject::builder(0x12345678u32, 0xABCDEF00u32)
            .property(0x1111, values::I32::new(42))
            .build()
    )
    .build();
```

See the [`ltk_meta` README](crates/ltk_meta/README.md) for the full surface: property paths, override bins (`PTCH`), typed values, and round-trip writing.

---

## 📚 Crates

| Crate | Description | Formats |
|-------|-------------|---------|
| [`league-toolkit`](https://crates.io/crates/league-toolkit) | Umbrella crate (feature-gated re-exports) | — |
| [`ltk_wad`](https://crates.io/crates/ltk_wad) | WAD archive reading/writing | `.wad.client` |
| [`ltk_texture`](https://crates.io/crates/ltk_texture) | Texture decoding/encoding | `.tex`, `.dds` |
| [`ltk_mesh`](https://crates.io/crates/ltk_mesh) | Skinned & static mesh parsing | `.skn`, `.scb`, `.sco` |
| [`ltk_anim`](https://crates.io/crates/ltk_anim) | Skeleton & animation formats | `.skl`, `.anm` |
| [`ltk_meta`](https://crates.io/crates/ltk_meta) | Property bin files | `.bin` |
| [`ltk_ritobin`](https://crates.io/crates/ltk_ritobin) | Human-readable bin format | ritobin text |
| [`ltk_mapgeo`](https://crates.io/crates/ltk_mapgeo) | Map environment geometry | `.mapgeo` |
| [`ltk_file`](https://crates.io/crates/ltk_file) | File type detection | — |
| [`ltk_hash`](https://crates.io/crates/ltk_hash) | Hash functions (FNV-1a, ELF) | — |
| [`ltk_shader`](https://crates.io/crates/ltk_shader) | Shader path utilities | — |
| [`ltk_primitives`](https://crates.io/crates/ltk_primitives) | Geometric primitives | — |
| [`ltk_io_ext`](https://crates.io/crates/ltk_io_ext) | I/O extensions (internal) | — |

Each crate lives under `crates/<name>`.

---

## ⚙️ Feature Flags

The `league-toolkit` umbrella crate uses feature flags to control which subsystems are included:

| Feature | Enables | Default |
|---------|---------|---------|
| `anim` | `ltk_anim` | ✅ |
| `file` | `ltk_file` | ✅ |
| `mesh` | `ltk_mesh` | ✅ |
| `meta` | `ltk_meta` | ✅ |
| `primitives` | `ltk_primitives` | ✅ |
| `texture` | `ltk_texture` | ✅ |
| `wad` | `ltk_wad` | ✅ |
| `hash` | `ltk_hash` | ✅ |
| `serde` | Serde support (where available) | ❌ |

For a minimal build, disable defaults and opt-in selectively:

```toml
[dependencies]
league-toolkit = { version = "0.2", default-features = false, features = ["wad"] }
```

Some crates expose their own feature flags — e.g. texture *encoding* requires `intel-tex` on `ltk_texture` (see the [`ltk_texture` README](crates/ltk_texture/README.md)).

---

## 📖 Documentation

- **[API Documentation](https://docs.rs/league-toolkit)** — Full rustdoc reference
- **[LTK Guide](docs/LTK_GUIDE.md)** — Comprehensive usage guide with examples

---

## 🛠️ Development

**Prerequisites:** Rust stable toolchain

```bash
# Build all crates
cargo build

# Run tests
cargo test

# Build documentation
cargo doc --open
```

### AI-Assisted Development

AI agents can produce large, hard-to-review changesets. This repository answers that with a
**document trail** rather than a tool pipeline: work is specified, decided and sliced in the repo
before it is written, and each artifact is reviewable on its own.

| Document | Holds | Where |
| --- | --- | --- |
| **PRD** | Why a feature exists, who asks for it, numbered requirements (`FR-N`) | `docs/prd/NNN-slug.md` |
| **ADR** | One architectural decision: what forced it, the options it beat, what it costs | `docs/adr/NNNN-slug.md` |
| **Design doc** | The API surface and the wire format | `docs/design/<feature>.md` |
| **Ticket** | One slice of implementable work, rendered to a GitHub issue | `.scratch/<project>/issues/*.md` |

The rule that keeps them readable: each cites the others rather than restating them. A design doc
cites requirements as `FR-N` and decisions as `ADR-NNNN`; two copies of one argument drift.

GitHub issues are **rendered** from the ticket files — the repo is the source of truth, and an
issue that disagrees with its ticket is fixed by re-rendering, not by editing it on GitHub.

Claude Code users get four skills in `.claude/skills/` that write and maintain all of this:
`write-prd`, `write-adr`, `write-ticket` and `sync-issues`. Worked example: PRD-001 with
ADR-0001 to ADR-0006 and `docs/design/ptch-property-patches.md`.

**Contributors using AI agents SHOULD follow this workflow.** A PR that arrives with no written
reasoning behind it may need extra review cycles. Day-to-day rules for agents live in
[`CLAUDE.md`](CLAUDE.md).

### Project Structure

```
league-toolkit/
|-- crates/
|   |-- league-toolkit/    # Umbrella crate
|   |-- ltk_wad/           # WAD archives
|   |-- ltk_texture/       # Textures
|   |-- ltk_mesh/          # Meshes
|   |-- ltk_anim/          # Animation
|   |-- ltk_meta/          # Property bins
|   |-- ltk_ritobin/       # Ritobin text format
|   |-- ltk_mapgeo/        # Map geometry
|   |-- ltk_file/          # File detection
|   |-- ltk_hash/          # Hashing
|   |-- ltk_shader/        # Shader utilities
|   |-- ltk_primitives/    # Primitives
|   |-- ltk_io_ext/        # I/O extensions
|-- docs/
    |-- LTK_GUIDE.md       # Usage guide
```

---

## 📋 Releasing

This repository uses [Release-plz](https://release-plz.ieni.dev/) for automated versioning and publishing:

1. Pushes to `main` trigger Release-plz to open a release PR
2. Merging the release PR publishes updated crates to crates.io

---

## 📄 License

Licensed under either of:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.

---

<div align="center">

Made with ❤️ by the [LeagueToolkit](https://github.com/LeagueToolkit) community

</div>
