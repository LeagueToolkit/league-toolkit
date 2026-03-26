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
ltk_texture = "0.4"
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
use std::io::Cursor;

let tex = Tex::from_reader(&mut cursor)?;
let surface = tex.decode_mipmap(0)?;
let image = surface.into_rgba_image()?;
image.save("output.png")?;
```

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
use ltk_meta::{BinTree, BinTreeObject, value::*};

// Read
let tree = BinTree::from_reader(&mut file)?;
for (hash, object) in &tree.objects {
    println!("Object 0x{:08x}", hash);
}

// Create
let tree = BinTree::builder()
    .dependency("shared/data.bin")
    .object(
        BinTreeObject::builder(0x12345678, 0xABCDEF00)
            .property(0x1111, I32Value(42))
            .build()
    )
    .build();
```

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

### Texture Encoding with `intel-tex`

BC1/BC3 texture encoding requires the optional `intel-tex` feature on `ltk_texture`:

```toml
[dependencies]
league-toolkit = { version = "0.2", features = ["texture"] }
ltk_texture = { version = "0.4", features = ["intel-tex"] }
```

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

### AI-Assisted Development with Speckit

This repository uses the **Speckit** workflow for developing new features and fixing bugs with AI agents (e.g., Claude Code). Speckit enforces a structured specification-plan-task pipeline so that all AI-generated work is auditable and reviewable before it lands in a PR.

**Why?** AI agents can produce large, hard-to-review changesets. Speckit breaks work into discrete, reviewable artifacts — a spec, a design plan, and an ordered task list — so maintainers can catch issues early and PRs stay focused.

**The workflow:**

1. **`/speckit.specify`** — Create or update a feature specification from a natural language description
2. **`/speckit.clarify`** — Identify underspecified areas and resolve ambiguities
3. **`/speckit.plan`** — Generate a design plan with architectural decisions
4. **`/speckit.tasks`** — Produce a dependency-ordered task list
5. **`/speckit.analyze`** — Cross-check consistency across spec, plan, and tasks
6. **`/speckit.implement`** — Execute the task list
7. **`/speckit.taskstoissues`** — Convert tasks into GitHub issues for tracking

All artifacts live in a `.specify/` directory scoped to the feature. The project's design principles are codified in [`.specify/memory/constitution.md`](.specify/memory/constitution.md).

**Contributors using AI agents SHOULD follow this workflow** to ensure that proposed changes are well-specified and easy to review. PRs generated without a spec/plan trail may require additional review cycles.

### Project Structure

```
league-toolkit/
├── crates/
│   ├── league-toolkit/    # Umbrella crate
│   ├── ltk_wad/           # WAD archives
│   ├── ltk_texture/       # Textures
│   ├── ltk_mesh/          # Meshes
│   ├── ltk_anim/          # Animation
│   ├── ltk_meta/          # Property bins
│   ├── ltk_ritobin/       # Ritobin text format
│   ├── ltk_mapgeo/        # Map geometry
│   ├── ltk_file/          # File detection
│   ├── ltk_hash/          # Hashing
│   ├── ltk_shader/        # Shader utilities
│   ├── ltk_primitives/    # Primitives
│   └── ltk_io_ext/        # I/O extensions
└── docs/
    └── LTK_GUIDE.md       # Usage guide
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
