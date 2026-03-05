# Game Data Server (GDS) — Implementation Plan

A local HTTP server built on top of `league-toolkit` that exposes League of Legends game data through a JSON API, enabling modders to query, modify, and build mods using a delta-based layer system inspired by [Riot's internal GDS](https://technology.riotgames.com/news/content-efficiency-game-data-server).

---

## Table of Contents

1. [Goals & Non-Goals](#1-goals--non-goals)
2. [Architecture Overview](#2-architecture-overview)
3. [Crate Structure](#3-crate-structure)
4. [Phase 1 — Core Library (`ltk-gds`)](#4-phase-1--core-library-ltk-gds)
5. [Phase 2 — Read-Only Data Server (`ltk-gds-server`)](#5-phase-2--read-only-data-server-ltk-gds-server)
6. [Phase 3 — Layer System](#6-phase-3--layer-system)
7. [Phase 4 — Build Pipeline](#7-phase-4--build-pipeline)
8. [Phase 5 — CLI Tool](#8-phase-5--cli-tool)
9. [Phase 6 — Advanced Features](#9-phase-6--advanced-features)
10. [API Reference](#10-api-reference)
11. [Data Model & Serialization](#11-data-model--serialization)
12. [Testing Strategy](#12-testing-strategy)
13. [Open Questions](#13-open-questions)

---

## 1. Goals & Non-Goals

### Goals

- Provide a **local HTTP API** for querying all game property data (bin objects) from WAD archives
- Implement a **layer system** for non-destructive, delta-based property modifications
- Support **hash ↔ name resolution** via community hash databases
- Build **mod artifacts** (`.wad.client` files) from layers + assets
- Follow existing workspace conventions (error handling, builder patterns, `from_reader`/`to_writer`)
- Simulate the content-creator workflow Riot uses internally, adapted for modding

### Non-Goals

- Game client patching / injection (out of scope — GDS produces files, not runtime modifications)
- Multiplayer / remote collaboration (local-only server; git handles collaboration)
- Full game client emulation
- GUI / web dashboard (Phase 6 stretch goal; CLI and API are primary interfaces)

---

## 2. Architecture Overview

```
┌──────────────────────────────────────────────────────────┐
│                  Consumer Tools                          │
│   CLI (ltk-gds)  │  curl/httpie  │  Web UI (future)     │
└────────┬─────────┴───────┬───────┴───────────────────────┘
         │  HTTP JSON API  │  localhost:1300
┌────────▼─────────────────▼───────────────────────────────┐
│               ltk-gds-server  (axum)                     │
│  ┌────────────┐ ┌────────────┐ ┌───────────────────────┐ │
│  │ REST Routes│ │ WebSocket  │ │ OpenAPI / Swagger      │ │
│  └─────┬──────┘ └─────┬──────┘ └───────────────────────┘ │
│        │               │                                  │
│  ┌─────▼───────────────▼─────────────────────────────┐   │
│  │             ltk-gds  (core library)               │   │
│  │  ┌───────────┐ ┌──────────┐ ┌──────────────────┐  │   │
│  │  │ Workspace │ │ Layer    │ │ Property         │  │   │
│  │  │ Manager   │ │ Engine   │ │ Registry         │  │   │
│  │  ├───────────┤ ├──────────┤ ├──────────────────┤  │   │
│  │  │ WAD Index │ │ Delta    │ │ Schema           │  │   │
│  │  │           │ │ Store    │ │ Registry         │  │   │
│  │  ├───────────┤ ├──────────┤ ├──────────────────┤  │   │
│  │  │ Hash DB   │ │ Conflict │ │ Build            │  │   │
│  │  │           │ │ Resolver │ │ Pipeline         │  │   │
│  │  └───────────┘ └──────────┘ └──────────────────┘  │   │
│  └───────────────────────────────────────────────────┘   │
└──────────────────────────┬───────────────────────────────┘
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐
  │ league-toolkit│  │ Mod Project │  │ Community    │
  │ (parsing)    │  │ (workspace) │  │ Hash DBs     │
  │ ltk_meta     │  │ layers/     │  │ (CDTB, etc.) │
  │ ltk_wad      │  │ assets/     │  │              │
  │ ltk_hash     │  │ output/     │  │              │
  │ ltk_ritobin  │  │             │  │              │
  │ ltk_file     │  │             │  │              │
  └─────────────┘  └─────────────┘  └──────────────┘
```

### Separation of Concerns

- **`ltk-gds`** (library crate): All core logic — workspace management, layer engine, property registry, build pipeline. No HTTP dependencies. Fully testable without a server.
- **`ltk-gds-server`** (binary crate): Thin HTTP layer over `ltk-gds`. Routes, serialization, WebSocket. Depends on `ltk-gds`.
- **`ltk-gds-cli`** (binary crate, or feature in server): CLI interface wrapping both library and server functionality.

This separation allows tools to embed the core library without pulling in axum/tokio.

---

## 3. Crate Structure

### New Workspace Members

```
crates/
├── ltk-gds/                    # Core library
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── error.rs            # GdsError enum (thiserror)
│       ├── workspace/          # Workspace management
│       │   ├── mod.rs
│       │   ├── config.rs       # gds.toml parsing
│       │   └── index.rs        # WAD content index
│       ├── registry/           # Property registry
│       │   ├── mod.rs
│       │   ├── object.rs       # Resolved object views
│       │   └── query.rs        # Query/filter engine
│       ├── layer/              # Layer system
│       │   ├── mod.rs
│       │   ├── delta.rs        # PropertyDelta type
│       │   ├── resolve.rs      # Layer resolution logic
│       │   └── conflict.rs     # Conflict detection
│       ├── hash_db/            # Hash database
│       │   ├── mod.rs
│       │   └── cdtb.rs         # CommunityDragon hash format
│       ├── schema/             # Schema inference
│       │   ├── mod.rs
│       │   └── infer.rs        # Schema extraction from bins
│       └── build/              # Build pipeline
│           ├── mod.rs
│           └── wad.rs          # WAD assembly
│
├── ltk-gds-server/             # HTTP server binary
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── state.rs            # Shared application state
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── objects.rs      # /api/objects/*
│       │   ├── layers.rs       # /api/layers/*
│       │   ├── assets.rs       # /api/assets/*
│       │   ├── schemas.rs      # /api/schemas/*
│       │   ├── hashes.rs       # /api/hashes/*
│       │   └── build.rs        # /api/build/*
│       ├── models/             # API request/response types
│       │   └── mod.rs
│       └── ws.rs               # WebSocket handler
```

### Dependency Graph

```
ltk-gds-server
  └── ltk-gds
        ├── ltk_meta (features = ["serde"])
        ├── ltk_wad (features = ["serde"])
        ├── ltk_file (features = ["serde"])
        ├── ltk_hash
        ├── ltk_ritobin
        └── ltk_primitives
```

### Cargo.toml — `ltk-gds`

```toml
[package]
name = "ltk-gds"
version = "0.1.0"
edition = "2021"

[dependencies]
# Workspace crates
ltk_meta = { path = "../ltk_meta", features = ["serde"] }
ltk_wad = { path = "../ltk_wad", features = ["serde"] }
ltk_file = { path = "../ltk_file", features = ["serde"] }
ltk_hash = { path = "../ltk_hash" }
ltk_ritobin = { path = "../ltk_ritobin" }
ltk_primitives = { path = "../ltk_primitives" }

# Workspace dependencies
thiserror = { workspace = true }
serde = { workspace = true }
indexmap = { workspace = true, features = ["serde"] }
glam = { workspace = true, features = ["serde"] }
log = { workspace = true }

# New dependencies (add to workspace)
serde_json = "1"
toml = "0.8"
camino = { workspace = true }
walkdir = "2"
xxhash-rust = { workspace = true }

[dev-dependencies]
insta = { workspace = true, features = ["json"] }
tempfile = "3"
```

### Cargo.toml — `ltk-gds-server`

```toml
[package]
name = "ltk-gds-server"
version = "0.1.0"
edition = "2021"

[dependencies]
ltk-gds = { path = "../ltk-gds" }

# Server
axum = "0.8"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
utoipa = { version = "5", features = ["axum_extras"] }
utoipa-swagger-ui = { version = "9", features = ["axum"] }

# Serialization
serde = { workspace = true }
serde_json = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# CLI
clap = { version = "4", features = ["derive"] }
```

---

## 4. Phase 1 — Core Library (`ltk-gds`)

This phase builds the foundational data structures and workspace management. No server yet — everything is a library.

### 4.1 Workspace Configuration

**File: `workspace/config.rs`**

The workspace is defined by a `gds.toml` file at the project root:

```toml
# gds.toml
[workspace]
name = "my-mod"

[game]
# Path to League install directory
path = "/path/to/Riot Games/League of Legends"
# Specific WAD files to mount (glob patterns, relative to game path)
# If omitted, mounts all .wad.client files
wad_patterns = ["DATA/FINAL/**/*.wad.client"]

[hashes]
# Paths to community hash databases
bin_hashes = ["hashes/hashes.binentries.txt", "hashes/hashes.binhashes.txt"]
wad_hashes = ["hashes/hashes.game.txt"]

[server]
port = 1300
```

```rust
// workspace/config.rs

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GdsConfig {
    pub workspace: WorkspaceConfig,
    pub game: GameConfig,
    #[serde(default)]
    pub hashes: HashConfig,
    #[serde(default)]
    pub server: ServerConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GameConfig {
    pub path: Utf8PathBuf,
    #[serde(default)]
    pub wad_patterns: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HashConfig {
    #[serde(default)]
    pub bin_hashes: Vec<Utf8PathBuf>,
    #[serde(default)]
    pub wad_hashes: Vec<Utf8PathBuf>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 { 1300 }
```

### 4.2 Hash Database

**File: `hash_db/mod.rs`**

Loads community hash databases (CommunityDragon CDTB format) for bidirectional hash ↔ name resolution.

**CDTB format** (one entry per line):
```
# hashes.binentries.txt
0a4e9e12 Characters/Aatrox/CharacterRecords/Root
1b5f0f23 Items/1001
```

```rust
// hash_db/mod.rs

use std::collections::HashMap;

/// Bidirectional hash ↔ name mapping
#[derive(Debug, Default)]
pub struct HashDatabase {
    /// FNV-1a (u32) bin hashes: object path hashes + property name hashes
    bin_to_name: HashMap<u32, String>,
    name_to_bin: HashMap<String, u32>,

    /// XXHash64 (u64) WAD path hashes
    wad_to_path: HashMap<u64, String>,
    path_to_wad: HashMap<String, u64>,
}

impl HashDatabase {
    pub fn new() -> Self { Self::default() }

    /// Load a CDTB-format hash file (hex_hash<space>name per line)
    pub fn load_bin_hashes(&mut self, path: &Path) -> Result<usize, GdsError>;
    pub fn load_wad_hashes(&mut self, path: &Path) -> Result<usize, GdsError>;

    /// Resolve bin hash → name, returning hex fallback if unknown
    pub fn resolve_bin(&self, hash: u32) -> &str;
    pub fn resolve_bin_opt(&self, hash: u32) -> Option<&str>;

    /// Resolve name → bin hash. Falls back to ltk_hash::fnv1a::hash_lower()
    pub fn hash_bin(&self, name: &str) -> u32 {
        self.name_to_bin.get(name)
            .copied()
            .unwrap_or_else(|| ltk_hash::fnv1a::hash_lower(name))
    }

    /// Resolve WAD path hash
    pub fn resolve_wad(&self, hash: u64) -> &str;
    pub fn resolve_wad_opt(&self, hash: u64) -> Option<&str>;

    /// Compute WAD path hash (XXHash64 of lowercased path)
    pub fn hash_wad(&self, path: &str) -> u64;

    /// Stats
    pub fn bin_count(&self) -> usize;
    pub fn wad_count(&self) -> usize;
}
```

### 4.3 WAD Index

**File: `workspace/index.rs`**

Scans a game installation, mounts all WADs, and builds an index mapping `path_hash → (wad_file, WadChunk)`.

```rust
// workspace/index.rs

use ltk_wad::{Wad, WadChunk};
use camino::Utf8PathBuf;

/// Index entry pointing to a specific chunk within a specific WAD file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WadIndexEntry {
    /// Which WAD file contains this chunk
    pub wad_path: Utf8PathBuf,
    /// Chunk metadata (offset, sizes, compression)
    pub chunk: WadChunk,
    /// Detected file type (if identifiable)
    pub file_kind: Option<LeagueFileKind>,
}

/// Full index of all content across all WAD files
#[derive(Debug, Default)]
pub struct WadIndex {
    /// path_hash (u64) → index entry
    entries: HashMap<u64, WadIndexEntry>,
    /// Reverse index: WAD file path → list of path_hashes it contains
    wad_contents: HashMap<Utf8PathBuf, Vec<u64>>,
}

impl WadIndex {
    /// Scan a game directory and index all WAD files matching patterns
    pub fn build(
        game_path: &Utf8Path,
        patterns: &[String],
        hash_db: &HashDatabase,
        on_progress: impl Fn(IndexProgress),
    ) -> Result<Self, GdsError>;

    /// Look up where a chunk lives
    pub fn get(&self, path_hash: u64) -> Option<&WadIndexEntry>;

    /// Look up by human-readable path (hashes through HashDatabase)
    pub fn get_by_path(&self, path: &str, hash_db: &HashDatabase) -> Option<&WadIndexEntry>;

    /// Load and decompress a chunk's data from its WAD file
    pub fn load_chunk_data(&self, path_hash: u64) -> Result<Box<[u8]>, GdsError>;

    /// List all indexed chunks
    pub fn iter(&self) -> impl Iterator<Item = (u64, &WadIndexEntry)>;

    /// List all indexed WAD files
    pub fn wad_files(&self) -> impl Iterator<Item = &Utf8Path>;

    /// Filter entries by file type
    pub fn entries_by_kind(&self, kind: LeagueFileKind) -> Vec<(u64, &WadIndexEntry)>;

    /// Total chunk count
    pub fn len(&self) -> usize;

    /// Serialize/deserialize for caching
    pub fn save_cache(&self, path: &Path) -> Result<(), GdsError>;
    pub fn load_cache(path: &Path) -> Result<Self, GdsError>;
}
```

### 4.4 Property Registry

**File: `registry/mod.rs`**

Loads all bin files from the WAD index into an in-memory queryable registry. This is the core data model.

```rust
// registry/mod.rs

use ltk_meta::{Bin, BinObject, BinProperty};
use ltk_meta::property::PropertyValueEnum;

/// A bin object annotated with its source location and resolved names
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedObject {
    /// Raw object data (from ltk_meta)
    pub object: BinObject,
    /// Human-readable path (from hash DB), if known
    pub path_name: Option<String>,
    /// Human-readable class name, if known
    pub class_name: Option<String>,
    /// Which bin file this object came from
    pub source_bin: String,
    /// Which WAD file the bin was in
    pub source_wad: Utf8PathBuf,
}

/// A single resolved property with metadata
#[derive(Debug, Clone, Serialize)]
pub struct ResolvedProperty {
    pub name_hash: u32,
    pub name: Option<String>,
    pub value: PropertyValueEnum,
}

/// In-memory registry of all bin objects from the game
#[derive(Debug)]
pub struct PropertyRegistry {
    /// All objects, keyed by path_hash
    objects: IndexMap<u32, ResolvedObject>,

    /// Class index: class_hash → [object path_hashes]
    class_index: HashMap<u32, Vec<u32>>,

    /// Source bin index: bin dependency path → Bin metadata
    bins: HashMap<String, BinMetadata>,
}

#[derive(Debug)]
struct BinMetadata {
    pub source_wad: Utf8PathBuf,
    pub is_override: bool,
    pub dependencies: Vec<String>,
    pub object_count: usize,
}

impl PropertyRegistry {
    /// Build registry by loading all PropertyBin chunks from the WAD index
    ///
    /// 1. Filters WadIndex for LeagueFileKind::PropertyBin entries
    /// 2. Loads and decompresses each chunk
    /// 3. Parses with Bin::from_reader()
    /// 4. Indexes all BinObjects by path_hash and class_hash
    /// 5. Resolves names via HashDatabase
    pub fn build(
        wad_index: &WadIndex,
        hash_db: &HashDatabase,
        on_progress: impl Fn(RegistryProgress),
    ) -> Result<Self, GdsError>;

    // --- Queries ---

    /// Get a single object by path hash
    pub fn get_object(&self, path_hash: u32) -> Option<&ResolvedObject>;

    /// Get a single object by human-readable path
    pub fn get_object_by_path(&self, path: &str, hash_db: &HashDatabase) -> Option<&ResolvedObject>;

    /// Get all objects of a given class
    pub fn objects_by_class(&self, class_hash: u32) -> &[u32];

    /// Get a specific property from an object
    pub fn get_property(&self, path_hash: u32, prop_hash: u32) -> Option<&BinProperty>;

    /// Search objects by path name pattern (glob-style)
    pub fn search(&self, pattern: &str, hash_db: &HashDatabase) -> Vec<&ResolvedObject>;

    /// Total object count
    pub fn len(&self) -> usize;

    /// List all unique class hashes
    pub fn classes(&self) -> impl Iterator<Item = u32>;
}
```

### 4.5 Error Type

**File: `error.rs`**

Following the workspace pattern of per-crate `thiserror` enums:

```rust
// error.rs

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GdsError {
    // Workspace errors
    #[error("workspace not found: no gds.toml in {0} or parent directories")]
    WorkspaceNotFound(String),
    #[error("invalid workspace config: {0}")]
    InvalidConfig(#[from] toml::de::Error),
    #[error("game directory not found: {0}")]
    GameNotFound(String),

    // WAD errors
    #[error("WAD error: {0}")]
    Wad(#[from] ltk_wad::WadError),
    #[error("chunk not found: {path_hash:#016x}")]
    ChunkNotFound { path_hash: u64 },

    // Bin/property errors
    #[error("bin parse error: {0}")]
    BinParse(#[from] ltk_meta::Error),
    #[error("object not found: {path_hash:#08x}")]
    ObjectNotFound { path_hash: u32 },
    #[error("property not found: object {object_hash:#08x}, property {property_hash:#08x}")]
    PropertyNotFound { object_hash: u32, property_hash: u32 },

    // Layer errors
    #[error("layer not found: {0}")]
    LayerNotFound(String),
    #[error("layer already exists: {0}")]
    LayerAlreadyExists(String),
    #[error("layer conflict: {0}")]
    LayerConflict(String),
    #[error("type mismatch: property {property_hash:#08x} expects {expected}, got {got}")]
    TypeMismatch {
        property_hash: u32,
        expected: String,
        got: String,
    },

    // Build errors
    #[error("build error: {0}")]
    Build(String),

    // Generic
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, GdsError>;
```

### 4.6 Workspace Manager

**File: `workspace/mod.rs`**

The top-level entry point that composes all subsystems:

```rust
// workspace/mod.rs

/// The main GDS workspace — owns all data and provides the public API
pub struct Workspace {
    config: GdsConfig,
    root: Utf8PathBuf,
    hash_db: HashDatabase,
    wad_index: WadIndex,
    registry: PropertyRegistry,
    layers: LayerEngine,       // Phase 3
    schemas: SchemaRegistry,   // Phase 6
}

impl Workspace {
    /// Open an existing workspace from a directory containing gds.toml
    pub fn open(path: impl AsRef<Utf8Path>) -> Result<Self, GdsError>;

    /// Initialize a new workspace
    pub fn init(
        path: impl AsRef<Utf8Path>,
        config: GdsConfig,
    ) -> Result<Self, GdsError>;

    /// Rebuild the WAD index and property registry (full re-scan)
    pub fn rebuild_index(
        &mut self,
        on_progress: impl Fn(Progress),
    ) -> Result<(), GdsError>;

    // --- Delegated accessors ---

    pub fn config(&self) -> &GdsConfig;
    pub fn hash_db(&self) -> &HashDatabase;
    pub fn wad_index(&self) -> &WadIndex;
    pub fn registry(&self) -> &PropertyRegistry;
    pub fn layers(&self) -> &LayerEngine;          // Phase 3
    pub fn layers_mut(&mut self) -> &mut LayerEngine;  // Phase 3
}
```

### Phase 1 Deliverables

| Component | Description | Key Dependencies |
|-----------|-------------|------------------|
| `GdsConfig` | TOML workspace configuration | `toml`, `serde`, `camino` |
| `HashDatabase` | Bidirectional hash ↔ name resolution | `ltk_hash` |
| `WadIndex` | Game content indexing across WAD files | `ltk_wad`, `ltk_file` |
| `PropertyRegistry` | In-memory bin object store with queries | `ltk_meta` |
| `GdsError` | Unified error type | `thiserror` |
| `Workspace` | Top-level coordinator | All above |

### Phase 1 Tests

- Unit: Hash database load/lookup with sample CDTB files
- Unit: WAD index build from test WAD files (use existing test fixtures if available, otherwise create minimal ones)
- Unit: Property registry build + query from in-memory `Bin` objects
- Integration: Full `Workspace::init()` → `rebuild_index()` round-trip with a small test fixture
- Snapshot: Registry query results serialized as JSON via `insta`

---

## 5. Phase 2 — Read-Only Data Server (`ltk-gds-server`)

### 5.1 Application State

```rust
// state.rs

use ltk_gds::Workspace;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type SharedState = Arc<RwLock<Workspace>>;
```

### 5.2 Server Bootstrap

```rust
// main.rs

use clap::Parser;

#[derive(Parser)]
#[command(name = "ltk-gds-server")]
struct Cli {
    /// Path to workspace directory (containing gds.toml)
    #[arg(short, long, default_value = ".")]
    workspace: String,

    /// Port to listen on (overrides gds.toml)
    #[arg(short, long)]
    port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::init();

    let cli = Cli::parse();
    let workspace = Workspace::open(&cli.workspace)?;
    let port = cli.port.unwrap_or(workspace.config().server.port);
    let state: SharedState = Arc::new(RwLock::new(workspace));

    let app = Router::new()
        .nest("/api", api_routes())
        .merge(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    tracing::info!("GDS listening on http://localhost:{port}");
    tracing::info!("Swagger docs at http://localhost:{port}/docs");
    axum::serve(listener, app).await?;
    Ok(())
}
```

### 5.3 Route Structure

```rust
// routes/mod.rs

pub fn api_routes() -> Router<SharedState> {
    Router::new()
        .nest("/objects", objects::routes())
        .nest("/hashes", hashes::routes())
        .nest("/assets", assets::routes())
        // Phase 3:
        // .nest("/layers", layers::routes())
        // Phase 4:
        // .nest("/build", build::routes())
        // Phase 6:
        // .nest("/schemas", schemas::routes())
}
```

### 5.4 Object Routes (Phase 2 scope)

```rust
// routes/objects.rs

/// GET /api/objects
/// Query parameters: ?class=<hash_or_name>&search=<pattern>&offset=0&limit=100
pub async fn list_objects(
    State(state): State<SharedState>,
    Query(params): Query<ObjectQuery>,
) -> Result<Json<ObjectListResponse>, ApiError>;

/// GET /api/objects/:path_or_hash
/// Accepts: hash (0xABCD1234) or human-readable path (Items/BlackCleaver)
pub async fn get_object(
    State(state): State<SharedState>,
    Path(path_or_hash): Path<String>,
) -> Result<Json<ObjectResponse>, ApiError>;

/// GET /api/objects/:path_or_hash/properties
/// Returns all properties of an object
pub async fn list_properties(
    State(state): State<SharedState>,
    Path(path_or_hash): Path<String>,
) -> Result<Json<Vec<PropertyResponse>>, ApiError>;

/// GET /api/objects/:path_or_hash/properties/:prop_name_or_hash
/// Returns a single property value
pub async fn get_property(
    State(state): State<SharedState>,
    Path((obj_path, prop_path)): Path<(String, String)>,
) -> Result<Json<PropertyResponse>, ApiError>;
```

### 5.5 API Response Models

```rust
// models/mod.rs

/// Unified identifier that accepts both hash and name
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum HashOrName {
    Hash(String),   // "0xABCD1234" format
    Name(String),   // Human-readable name
}

impl HashOrName {
    /// Resolve to a u32 bin hash using the hash database
    pub fn resolve_bin(&self, hash_db: &HashDatabase) -> Result<u32, ApiError>;
    /// Resolve to a u64 WAD hash
    pub fn resolve_wad(&self, hash_db: &HashDatabase) -> Result<u64, ApiError>;
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ObjectResponse {
    pub path_hash: String,       // "0xABCD1234"
    pub path_name: Option<String>,
    pub class_hash: String,
    pub class_name: Option<String>,
    pub source_bin: String,
    pub source_wad: String,
    pub properties: Vec<PropertyResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct PropertyResponse {
    pub name_hash: String,
    pub name: Option<String>,
    /// Serde-serialized PropertyValueEnum
    /// Uses the existing tag = "kind", content = "value" format from ltk_meta
    pub value: serde_json::Value,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ObjectListResponse {
    pub objects: Vec<ObjectSummary>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ObjectSummary {
    pub path_hash: String,
    pub path_name: Option<String>,
    pub class_hash: String,
    pub class_name: Option<String>,
    pub property_count: usize,
}
```

### 5.6 Hash Routes

```rust
// routes/hashes.rs

/// GET /api/hashes/lookup?name=<name>&type=<bin|wad>
/// Compute hash from name
pub async fn lookup_hash(...) -> Result<Json<HashLookupResponse>, ApiError>;

/// GET /api/hashes/reverse?hash=<hash>&type=<bin|wad>
/// Look up name from hash
pub async fn reverse_hash(...) -> Result<Json<HashLookupResponse>, ApiError>;

/// POST /api/hashes/import
/// Import a CDTB hash file
pub async fn import_hashes(...) -> Result<Json<ImportResult>, ApiError>;

/// GET /api/hashes/stats
/// Hash database statistics
pub async fn hash_stats(...) -> Result<Json<HashStats>, ApiError>;
```

### 5.7 Asset Routes

```rust
// routes/assets.rs

/// GET /api/assets/:path_or_hash
/// Load raw decompressed chunk data from WAD
pub async fn get_asset(
    State(state): State<SharedState>,
    Path(path_or_hash): Path<String>,
) -> Result<Response<Body>, ApiError>;

/// GET /api/assets/:path_or_hash/info
/// Get metadata about an asset (type, size, WAD source)
pub async fn get_asset_info(...) -> Result<Json<AssetInfo>, ApiError>;

/// GET /api/assets/search?type=<file_kind>&q=<path_pattern>
/// Search assets by type and path
pub async fn search_assets(...) -> Result<Json<Vec<AssetInfo>>, ApiError>;
```

### Phase 2 Deliverables

| Component | Description |
|-----------|-------------|
| Axum server with object/hash/asset routes | Read-only JSON API |
| OpenAPI/Swagger documentation | Auto-generated from `utoipa` annotations |
| Pagination for list endpoints | Offset/limit query parameters |
| Dual hash/name resolution in all path parameters | `0xABCD` or `Items/BlackCleaver` |
| CORS enabled | For web-based tool access |
| Structured JSON error responses | Consistent error format |

### Phase 2 Tests

- Integration: Start server, query objects, verify JSON structure
- Integration: Hash lookup/reverse round-trip
- Integration: Asset download returns correct bytes
- Integration: Pagination (offset/limit)
- Integration: 404 for missing objects
- Snapshot: API response JSON shapes via `insta`

---

## 6. Phase 3 — Layer System

The layer system is the core differentiator. It enables non-destructive, composable property modifications.

### 6.1 Data Model

```rust
// layer/mod.rs

use ltk_meta::property::PropertyValueEnum;

/// A layer represents a named set of property modifications
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub name: String,
    pub description: String,
    pub priority: u32,
    pub enabled: bool,
    /// When this layer was created
    pub created_at: String,  // ISO 8601
}

/// Persistent layer storage on disk
///
/// Layout:
///   layers/{name}/
///     meta.toml          # Layer struct (serialized)
///     deltas/
///       {path_hash}.json # One file per modified object
#[derive(Debug)]
pub struct LayerStore {
    root: Utf8PathBuf,
    layers: IndexMap<String, Layer>,
}
```

### 6.2 Delta Format

```rust
// layer/delta.rs

/// Records all modifications to a single BinObject within a layer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectDelta {
    /// Object identification
    pub path_hash: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_name: Option<String>,
    pub class_hash: u32,

    /// Is this an entirely new object (not in base game)?
    pub is_new: bool,

    /// Properties that were modified (hash → new value)
    /// Stored alongside original value for diff/undo
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub modified: IndexMap<u32, PropertyModification>,

    /// Properties that were added (not in base object)
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub added: IndexMap<u32, PropertyValueEnum>,

    /// Properties that were removed (present in base, deleted by this layer)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<u32>,
}

/// A single property modification with before/after for conflict detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyModification {
    /// Value before this layer's modification (from base or lower-priority layer)
    pub original: PropertyValueEnum,
    /// New value set by this layer
    pub modified: PropertyValueEnum,
}
```

**On-disk delta example** (`layers/balance-patch/deltas/0xa1b2c3d4.json`):

```json
{
  "path_hash": "0xa1b2c3d4",
  "path_name": "Items/BlackCleaver",
  "class_hash": "0x12345678",
  "is_new": false,
  "modified": {
    "0xaabbccdd": {
      "original": { "kind": "F32", "value": { "value": 450.0 } },
      "modified": { "kind": "F32", "value": { "value": 500.0 } }
    }
  },
  "added": {},
  "removed": []
}
```

> **Note on serde format**: The `PropertyValueEnum` serialization uses the existing
> `#[serde(tag = "kind", content = "value")]` attribute from `ltk_meta`.
> This produces `{"kind": "F32", "value": {"value": 450.0}}`.
> If a cleaner JSON format is needed for the API, a separate `ApiPropertyValue`
> type can be introduced as a thin mapping layer without modifying `ltk_meta`.

### 6.3 Layer Engine

```rust
// layer/resolve.rs

/// Engine that manages layers and computes resolved property state
#[derive(Debug)]
pub struct LayerEngine {
    store: LayerStore,
    /// Active layer being edited (if any)
    active_layer: Option<String>,
}

impl LayerEngine {
    pub fn new(workspace_root: &Utf8Path) -> Result<Self, GdsError>;

    // --- Layer CRUD ---

    pub fn create_layer(&mut self, name: &str, description: &str) -> Result<&Layer, GdsError>;
    pub fn delete_layer(&mut self, name: &str) -> Result<(), GdsError>;
    pub fn get_layer(&self, name: &str) -> Option<&Layer>;
    pub fn list_layers(&self) -> &IndexMap<String, Layer>;
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> Result<(), GdsError>;
    pub fn set_priority(&mut self, name: &str, priority: u32) -> Result<(), GdsError>;
    pub fn reorder(&mut self, order: &[String]) -> Result<(), GdsError>;

    // --- Active layer operations ---

    /// Set which layer new modifications go into
    pub fn set_active(&mut self, name: &str) -> Result<(), GdsError>;
    pub fn active_layer(&self) -> Option<&str>;

    /// Modify a property in the active layer
    ///
    /// 1. Looks up original value from the registry (base game)
    /// 2. Creates or updates the ObjectDelta for this object
    /// 3. Records the PropertyModification with original + modified values
    /// 4. Persists the delta to disk
    pub fn set_property(
        &mut self,
        path_hash: u32,
        prop_hash: u32,
        value: PropertyValueEnum,
        registry: &PropertyRegistry,
    ) -> Result<(), GdsError>;

    /// Remove a property override (revert to base)
    pub fn remove_override(
        &mut self,
        path_hash: u32,
        prop_hash: u32,
    ) -> Result<(), GdsError>;

    /// Add an entirely new object
    pub fn add_object(
        &mut self,
        object: BinObject,
    ) -> Result<(), GdsError>;

    // --- Resolution ---

    /// Resolve the final state of an object after all enabled layers are applied
    ///
    /// Algorithm:
    /// 1. Start with the base object from PropertyRegistry
    /// 2. For each enabled layer (sorted by priority ascending):
    ///    a. If the layer has an ObjectDelta for this path_hash:
    ///       - Apply modified properties (overwrite)
    ///       - Apply added properties (insert)
    ///       - Apply removed properties (delete)
    /// 3. Return the resolved BinObject
    pub fn resolve_object(
        &self,
        path_hash: u32,
        registry: &PropertyRegistry,
    ) -> Result<BinObject, GdsError>;

    /// Get the diff for a specific layer (what it changes from base)
    pub fn layer_diff(&self, name: &str) -> Result<Vec<ObjectDelta>, GdsError>;

    // --- Conflict detection ---

    /// Check for conflicts between two layers
    /// A conflict exists when both layers modify the same property on the same object
    pub fn check_conflicts(
        &self,
        layer_a: &str,
        layer_b: &str,
    ) -> Result<Vec<Conflict>, GdsError>;

    /// Check a layer against all other enabled layers
    pub fn check_all_conflicts(&self, name: &str) -> Result<Vec<Conflict>, GdsError>;
}

/// A conflict between two layers modifying the same property
#[derive(Debug, Serialize)]
pub struct Conflict {
    pub object_path_hash: u32,
    pub object_path_name: Option<String>,
    pub property_hash: u32,
    pub property_name: Option<String>,
    pub layer_a: String,
    pub layer_a_value: PropertyValueEnum,
    pub layer_b: String,
    pub layer_b_value: PropertyValueEnum,
}
```

### 6.4 Layer API Routes

```rust
// routes/layers.rs

/// GET    /api/layers                       → list all layers
/// POST   /api/layers                       → create layer {name, description}
/// GET    /api/layers/:name                 → get layer metadata
/// PUT    /api/layers/:name                 → update layer config
/// DELETE /api/layers/:name                 → delete layer
/// POST   /api/layers/:name/enable          → enable layer
/// POST   /api/layers/:name/disable         → disable layer
/// POST   /api/layers/reorder               → set priority order [{name, priority}]
/// POST   /api/layers/:name/activate        → set as active layer for edits
/// GET    /api/layers/:name/diff            → show all deltas in this layer
/// GET    /api/layers/:name/conflicts       → check conflicts with other layers
///
/// PUT    /api/objects/:path/properties/:prop  → set property (writes to active layer)
/// DELETE /api/objects/:path/properties/:prop  → remove property override
/// POST   /api/objects                         → create new object in active layer
```

### Phase 3 Deliverables

| Component | Description |
|-----------|-------------|
| `Layer` + `LayerStore` | CRUD + disk persistence in `layers/` directory |
| `ObjectDelta` + `PropertyModification` | JSON delta format with original/modified tracking |
| `LayerEngine` | Resolution, conflict detection, active layer management |
| Layer API routes | Full REST interface for layer management |
| Property mutation routes | SET/DELETE on properties via active layer |

### Phase 3 Tests

- Unit: Layer create/delete/enable/disable
- Unit: Delta creation from property set operations
- Unit: Layer resolution with single and multiple layers
- Unit: Conflict detection between overlapping layers
- Unit: Priority ordering behavior
- Integration: Layer CRUD through API
- Integration: Set property → get resolved object → verify value
- Snapshot: Delta JSON format via `insta`
- Round-trip: Set property → delete layer → verify base state restored

---

## 7. Phase 4 — Build Pipeline

Transforms layers + assets into deployable `.wad.client` mod files.

### 7.1 Build Engine

```rust
// build/mod.rs

/// Configuration for a build
#[derive(Debug, Serialize, Deserialize)]
pub struct BuildConfig {
    /// Which layers to include (default: all enabled layers)
    pub layers: Option<Vec<String>>,
    /// Output directory
    pub output_dir: Utf8PathBuf,
    /// Output filename (default: {workspace_name}.wad.client)
    pub output_name: Option<String>,
    /// Compression for new/modified chunks
    pub compression: WadChunkCompression,
}

/// Result of a build operation
#[derive(Debug, Serialize)]
pub struct BuildResult {
    pub output_path: Utf8PathBuf,
    pub chunks_written: usize,
    pub modified_objects: usize,
    pub new_objects: usize,
    pub assets_included: usize,
    pub total_size: u64,
}

/// Progress reporting for builds
#[derive(Debug, Serialize)]
pub struct BuildProgress {
    pub stage: BuildStage,
    pub current: usize,
    pub total: usize,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub enum BuildStage {
    ResolvingLayers,
    SerializingBins,
    CopyingAssets,
    CompressingChunks,
    WritingWad,
}
```

### 7.2 Build Algorithm

```rust
// build/wad.rs

/// Build a mod WAD file from workspace layers and assets
///
/// Algorithm:
/// 1. Resolve all enabled layers to get final BinObject states
/// 2. Group modified objects by their source bin file
/// 3. For each modified bin file:
///    a. Clone the base Bin from the registry
///    b. Apply all object modifications (replace/add/remove)
///    c. Serialize to binary using Bin::to_writer()
///    d. Register as a WadChunkBuilder entry
/// 4. Scan the assets/ directory for non-bin mod assets
/// 5. Register each asset as a WadChunkBuilder entry
/// 6. Build the WAD using WadBuilder::build_to_writer()
pub fn build_mod_wad(
    workspace: &Workspace,
    config: &BuildConfig,
    on_progress: impl Fn(BuildProgress),
) -> Result<BuildResult, GdsError> {
    let engine = workspace.layers();
    let registry = workspace.registry();
    let hash_db = workspace.hash_db();

    // Step 1: Collect all deltas from enabled layers
    let all_deltas = engine.collect_enabled_deltas()?;

    // Step 2: Group by source bin
    let mut bins_to_rebuild: HashMap<String, Bin> = HashMap::new();
    for (path_hash, _delta) in &all_deltas {
        let resolved = engine.resolve_object(*path_hash, registry)?;
        let source_bin = registry.get_object(*path_hash)
            .map(|o| o.source_bin.clone())
            .unwrap_or_else(|| "new.bin".into());

        bins_to_rebuild
            .entry(source_bin)
            .or_insert_with(|| /* clone base bin or create new */)
            .objects
            .insert(*path_hash, resolved);
    }

    // Step 3: Serialize bins and register chunks
    let mut builder = WadBuilder::default();
    for (bin_path, bin) in &bins_to_rebuild {
        let wad_path_hash = hash_db.hash_wad(bin_path);
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_path(bin_path)
                .with_force_compression(config.compression)
        );
    }

    // Step 4: Add asset files
    let assets_dir = workspace.root().join("assets");
    if assets_dir.exists() {
        for entry in walkdir::WalkDir::new(&assets_dir) {
            // Register each asset as a chunk
        }
    }

    // Step 5: Write WAD
    let output_path = config.output_dir.join(
        config.output_name.as_deref().unwrap_or("mod.wad.client")
    );
    let mut output_file = File::create(&output_path)?;
    builder.build_to_writer(&mut output_file, |path_hash, cursor| {
        // Provide data for each chunk (serialized bin or raw asset bytes)
    })?;

    Ok(BuildResult { /* ... */ })
}
```

### 7.3 Build API Routes

```rust
// routes/build.rs

/// POST /api/build/wad
/// Body: BuildConfig (optional, uses defaults)
/// Triggers a build and returns the result
pub async fn build_wad(...) -> Result<Json<BuildResult>, ApiError>;

/// POST /api/build/preview
/// Dry-run: returns what would be built without writing anything
pub async fn preview_build(...) -> Result<Json<BuildPreview>, ApiError>;

/// POST /api/build/validate
/// Validates all layers and assets before building
pub async fn validate_build(...) -> Result<Json<ValidationResult>, ApiError>;
```

### Phase 4 Deliverables

| Component | Description |
|-----------|-------------|
| `BuildConfig` + `BuildResult` | Build configuration and output reporting |
| `build_mod_wad()` | Core build algorithm: resolve → serialize → pack |
| Asset scanning | Walk `assets/` directory and include in WAD |
| Build API routes | `/api/build/wad`, `/api/build/preview`, `/api/build/validate` |
| Progress reporting | `BuildProgress` enum with stage tracking |

### Phase 4 Tests

- Unit: Resolve layers → serialize bins → verify binary round-trip with `Bin::from_reader()`
- Unit: Build preview produces correct chunk list without I/O
- Integration: Full build pipeline from test layers → `.wad.client` → verify contents
- Integration: Asset inclusion (add file to `assets/`, verify it appears in WAD)
- Round-trip: Build WAD → mount with `Wad::mount()` → extract bins → compare to expected

---

## 8. Phase 5 — CLI Tool

### 8.1 Command Structure

```
ltk-gds <COMMAND>

Commands:
  init          Initialize a new GDS workspace
  serve         Start the HTTP server
  index         Rebuild the WAD/property index
  hash          Hash utilities

  layer         Layer management
    create      Create a new layer
    delete      Delete a layer
    list        List all layers
    enable      Enable a layer
    disable     Disable a layer
    diff        Show changes in a layer
    conflicts   Check for conflicts

  get           Query game data
    object      Get a bin object by path/hash
    property    Get a specific property
    class       List objects of a class

  set           Modify game data (writes to active layer)
    property    Set a property value
    active      Set the active layer

  build         Build mod artifacts
    wad         Build .wad.client from layers
    preview     Preview what would be built
    validate    Validate layers and assets

  export        Export data
    ritobin     Export objects as ritobin text format
    json        Export objects as JSON
```

### 8.2 Key CLI Flows

```bash
# Initialize workspace
ltk-gds init --game-path "/path/to/League" --name "my-mod"
# Creates gds.toml, directories, downloads default hash DBs

# Build index (first run or after game update)
ltk-gds index
# Mounts WADs, parses bins, builds property registry
# Displays: "Indexed 47 WAD files, 182,341 bin objects, 4.2M properties"

# Start server
ltk-gds serve
# "GDS listening on http://localhost:1300"
# "Swagger docs at http://localhost:1300/docs"

# Query data via CLI (no server needed)
ltk-gds get object "Items/BlackCleaver"
ltk-gds get object 0xa1b2c3d4
ltk-gds get class ItemData --limit 10
ltk-gds get property "Items/BlackCleaver" mFlatHPMod

# Layer management
ltk-gds layer create "balance-patch" --description "HP buffs for Season 15"
ltk-gds set active "balance-patch"
ltk-gds set property "Items/BlackCleaver" mFlatHPMod 500.0 --type f32
ltk-gds layer diff "balance-patch"
ltk-gds layer conflicts "balance-patch"

# Build
ltk-gds build wad --output ./output/
ltk-gds build preview
ltk-gds build validate

# Export
ltk-gds export ritobin "Items/*" > items.ritobin.txt
ltk-gds export json --class ItemData > items.json

# Hash utilities
ltk-gds hash bin "mSpellName"        # → 0xaabbccdd
ltk-gds hash wad "items/blackcleaver.bin"  # → 0x1234567890abcdef
ltk-gds hash reverse 0xaabbccdd      # → "mSpellName"
```

### Phase 5 Implementation

The CLI crate can either be:
- **A)** A separate `ltk-gds-cli` crate that depends on `ltk-gds` (core lib)
- **B)** A `cli` feature/binary in `ltk-gds-server` that adds `clap` commands

**Recommendation**: Option B — single `ltk-gds-server` binary with both `serve` and direct query commands. The direct query commands (`get`, `set`, `layer`, `build`) work without starting the server by directly using the `ltk-gds` library.

---

## 9. Phase 6 — Advanced Features

### 9.1 Schema Registry

Infer class schemas by analyzing all bin objects in the game:

```rust
// schema/mod.rs

/// Inferred schema for a bin class
#[derive(Debug, Clone, Serialize)]
pub struct ClassSchema {
    pub class_hash: u32,
    pub class_name: Option<String>,
    pub properties: IndexMap<u32, PropertySchema>,
    /// Number of objects using this class
    pub instance_count: usize,
}

/// Inferred schema for a single property
#[derive(Debug, Clone, Serialize)]
pub struct PropertySchema {
    pub name_hash: u32,
    pub name: Option<String>,
    /// The property type (Kind from ltk_meta)
    pub kind: Kind,
    /// For container/map types, the element type(s)
    pub element_kind: Option<Kind>,
    pub map_key_kind: Option<Kind>,
    /// How many objects have this property (frequency)
    pub frequency: usize,
    /// Sample values (first N distinct values seen)
    pub sample_values: Vec<PropertyValueEnum>,
}

#[derive(Debug)]
pub struct SchemaRegistry {
    schemas: IndexMap<u32, ClassSchema>,
}

impl SchemaRegistry {
    /// Build schemas by scanning all objects in the property registry
    ///
    /// For each BinObject:
    ///   1. Get or create ClassSchema for its class_hash
    ///   2. For each BinProperty:
    ///      a. Get or create PropertySchema for its name_hash
    ///      b. Record the Kind, increment frequency
    ///      c. Optionally collect sample values
    pub fn build(registry: &PropertyRegistry, hash_db: &HashDatabase) -> Self;

    pub fn get_class(&self, class_hash: u32) -> Option<&ClassSchema>;
    pub fn search_classes(&self, pattern: &str) -> Vec<&ClassSchema>;
    pub fn validate_property(
        &self,
        class_hash: u32,
        prop_hash: u32,
        value: &PropertyValueEnum,
    ) -> Result<(), ValidationError>;
}
```

**API Routes:**

```
GET /api/schemas                    → list all classes
GET /api/schemas/:class             → get class schema
GET /api/schemas/:class/instances   → list objects of this class
POST /api/validate                  → validate a value against schema
```

### 9.2 WebSocket Live Reload

```rust
// ws.rs

/// WebSocket endpoint for real-time change notifications
///
/// Events:
/// - property_changed { object, property, layer, old_value, new_value }
/// - layer_created { name }
/// - layer_deleted { name }
/// - layer_enabled { name }
/// - layer_disabled { name }
/// - build_progress { stage, current, total }
/// - build_complete { result }
///
/// ws://localhost:1300/ws
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<SharedState>,
) -> impl IntoResponse;
```

### 9.3 Ritobin Integration

Leverage `ltk_ritobin` for human-readable import/export:

```
GET  /api/export/ritobin?objects=0x1234,0x5678    → text/plain ritobin
GET  /api/export/ritobin?class=ItemData            → all items as ritobin
POST /api/import/ritobin                           → parse ritobin text, apply as layer
```

Implementation uses existing `ltk_ritobin::write()` and `ltk_ritobin::parse_to_bin_tree()`.

### 9.4 Filesystem Watcher

Watch the `layers/` and `assets/` directories for external changes:

```rust
// Uses notify crate
let (tx, rx) = channel();
let mut watcher = notify::recommended_watcher(tx)?;
watcher.watch(workspace.root().join("layers"), RecursiveMode::Recursive)?;
watcher.watch(workspace.root().join("assets"), RecursiveMode::Recursive)?;

// On change: reload affected layers, broadcast via WebSocket
```

This enables round-tripping with external editors: edit a delta JSON file in VS Code → GDS picks up the change → WebSocket notifies connected tools.

### 9.5 Template System

Pre-built templates for common modding tasks:

```
ltk-gds template list
ltk-gds template apply new-champion --name "MyChamp"
ltk-gds template apply item-rebalance --item "BlackCleaver"
```

Templates are JSON files defining a set of object deltas with placeholder values.

---

## 10. API Reference

### Full Route Map

| Method | Path | Phase | Description |
|--------|------|-------|-------------|
| `GET` | `/api/objects` | 2 | List/search objects |
| `GET` | `/api/objects/:id` | 2 | Get object |
| `GET` | `/api/objects/:id/properties` | 2 | List properties |
| `GET` | `/api/objects/:id/properties/:prop` | 2 | Get property |
| `PUT` | `/api/objects/:id/properties/:prop` | 3 | Set property (via active layer) |
| `DELETE` | `/api/objects/:id/properties/:prop` | 3 | Remove property override |
| `POST` | `/api/objects` | 3 | Create new object |
| | | | |
| `GET` | `/api/layers` | 3 | List layers |
| `POST` | `/api/layers` | 3 | Create layer |
| `GET` | `/api/layers/:name` | 3 | Get layer |
| `PUT` | `/api/layers/:name` | 3 | Update layer |
| `DELETE` | `/api/layers/:name` | 3 | Delete layer |
| `POST` | `/api/layers/:name/enable` | 3 | Enable layer |
| `POST` | `/api/layers/:name/disable` | 3 | Disable layer |
| `POST` | `/api/layers/:name/activate` | 3 | Set active layer |
| `POST` | `/api/layers/reorder` | 3 | Reorder priorities |
| `GET` | `/api/layers/:name/diff` | 3 | Layer diff |
| `GET` | `/api/layers/:name/conflicts` | 3 | Conflict check |
| | | | |
| `GET` | `/api/assets/:path` | 2 | Get raw asset data |
| `GET` | `/api/assets/:path/info` | 2 | Asset metadata |
| `GET` | `/api/assets/search` | 2 | Search assets |
| `POST` | `/api/assets/import` | 4 | Import asset file |
| | | | |
| `GET` | `/api/hashes/lookup` | 2 | Name → hash |
| `GET` | `/api/hashes/reverse` | 2 | Hash → name |
| `POST` | `/api/hashes/import` | 2 | Import hash DB |
| `GET` | `/api/hashes/stats` | 2 | Hash DB stats |
| | | | |
| `POST` | `/api/build/wad` | 4 | Build mod WAD |
| `POST` | `/api/build/preview` | 4 | Dry-run preview |
| `POST` | `/api/build/validate` | 4 | Validate before build |
| | | | |
| `GET` | `/api/schemas` | 6 | List class schemas |
| `GET` | `/api/schemas/:class` | 6 | Get class schema |
| | | | |
| `GET` | `/api/export/ritobin` | 6 | Export as ritobin text |
| `POST` | `/api/import/ritobin` | 6 | Import ritobin text |
| | | | |
| `WS` | `/ws` | 6 | Live change notifications |

### Error Format

All errors return a consistent JSON structure:

```json
{
  "error": {
    "code": "OBJECT_NOT_FOUND",
    "message": "Object not found: 0xa1b2c3d4",
    "details": null
  }
}
```

HTTP status codes: `400` (bad request), `404` (not found), `409` (conflict), `500` (internal).

---

## 11. Data Model & Serialization

### Leveraging Existing Serde Support

`ltk_meta` already provides conditional serde support behind the `"serde"` feature flag. Key serialization behaviors:

| Type | Serde Behavior | Notes |
|------|----------------|-------|
| `Bin` | Direct serialize/deserialize | `IndexMap<u32, BinObject>` preserves insertion order |
| `BinObject` | Direct | `path_hash`, `class_hash`, `properties` |
| `BinProperty` | `#[serde(flatten)]` on value | Flattens `PropertyValueEnum` into property |
| `PropertyValueEnum` | `#[serde(tag = "kind", content = "value")]` | Tagged enum: `{"kind": "F32", "value": {...}}` |
| Primitives (I32, F32, etc.) | `{"value": <T>, "meta": ...}` | Meta is `NoMeta` (unit) by default |
| `WadChunk` | Direct | All fields serializable |
| `WadChunks` | Serializes `chunks` vec | `index` HashMap is `#[serde(skip)]` |

### glam Serde

When `ltk_meta/serde` is enabled, it also enables `glam/serde`. glam vectors serialize as arrays:

```json
{ "kind": "Vector3", "value": { "value": [1.0, 2.0, 3.0] } }
```

### API Property Value Format

For the REST API, we add a thin mapping layer that produces cleaner JSON:

```rust
// models/mod.rs

/// Simplified property representation for API responses
#[derive(Debug, Serialize, ToSchema)]
#[serde(tag = "type")]
pub enum ApiPropertyValue {
    #[serde(rename = "bool")]
    Bool { value: bool },
    #[serde(rename = "i32")]
    I32 { value: i32 },
    #[serde(rename = "f32")]
    F32 { value: f32 },
    #[serde(rename = "string")]
    String { value: String },
    #[serde(rename = "vec3")]
    Vec3 { value: [f32; 3] },
    #[serde(rename = "hash")]
    Hash { value: String, name: Option<String> },
    #[serde(rename = "object_link")]
    ObjectLink { value: String, name: Option<String> },
    // ... etc
}

impl ApiPropertyValue {
    /// Convert from ltk_meta's PropertyValueEnum + HashDatabase
    pub fn from_property_value(
        value: &PropertyValueEnum,
        hash_db: &HashDatabase,
    ) -> Self;

    /// Convert back to PropertyValueEnum for storage
    pub fn to_property_value(&self) -> Result<PropertyValueEnum, GdsError>;
}
```

This produces API responses like:

```json
{
  "name_hash": "0xaabbccdd",
  "name": "mFlatHPMod",
  "type": "f32",
  "value": 450.0
}
```

Instead of the raw serde format:

```json
{
  "name_hash": 2864434397,
  "kind": "F32",
  "value": { "value": 450.0, "meta": null }
}
```

---

## 12. Testing Strategy

### Test Fixtures

Create minimal test data under `crates/ltk-gds/tests/fixtures/`:

```
tests/fixtures/
├── minimal.wad.client      # WAD with a few bin + asset chunks
├── test.bin                 # Small bin file with known objects
├── hashes.binentries.txt   # Hash DB for test objects
├── hashes.game.txt         # WAD path hashes for test data
└── workspace/              # Pre-built workspace for integration tests
    ├── gds.toml
    ├── layers/
    │   └── test-layer/
    │       ├── meta.toml
    │       └── deltas/
    └── assets/
```

### Test Matrix

| Phase | Test Type | What's Tested |
|-------|-----------|---------------|
| 1 | Unit | Hash DB load, WAD indexing, property registry queries |
| 1 | Snapshot | Registry query results as JSON |
| 2 | Integration | HTTP routes return correct JSON, status codes |
| 2 | Integration | Hash/name duality in URL paths |
| 3 | Unit | Layer CRUD, delta creation, resolution algorithm |
| 3 | Unit | Conflict detection between layers |
| 3 | Integration | Set property → query → verify modified value |
| 3 | Round-trip | Set → get → remove override → get → verify base restored |
| 4 | Integration | Build WAD → mount → verify contents match expectations |
| 4 | Round-trip | Modify → build → parse WAD → compare to expected objects |
| 6 | Unit | Schema inference from sample objects |
| 6 | Unit | Property validation against inferred schemas |

### Snapshot Testing

Use `insta` with JSON format for API response snapshots:

```rust
#[test]
fn test_object_response_format() {
    let object = create_test_object();
    let response = ObjectResponse::from_resolved(object, &hash_db);
    insta::assert_json_snapshot!(response);
}
```

---

## 13. Open Questions

### Design Decisions Needed

1. **Index caching strategy**: Should the WAD index + property registry be serialized to disk for fast subsequent startups? Options:
   - (A) SQLite database for indexed queries
   - (B) bincode/MessagePack serialized cache file
   - (C) Always rebuild from WAD files (slower startup, simpler)

2. **Multi-WAD conflict handling**: When the same `path_hash` exists in multiple WAD files (e.g., patch WADs override base WADs), which takes precedence? Need to investigate Riot's WAD layering order.

3. **Bin file grouping in build output**: Should the build pipeline:
   - (A) Create one WAD per modified bin file (granular patching)
   - (B) Create a single WAD with all modifications (simpler)
   - (C) Mirror the game's WAD structure (most compatible)

4. **Delta format versioning**: Should `ObjectDelta` JSON include a format version for forward compatibility?

5. **Async vs sync in core library**: The core library (`ltk-gds`) is sync (matching league-toolkit conventions). The server wraps sync operations in `tokio::task::spawn_blocking()`. Is this sufficient, or should the core library offer async variants?

6. **Hash database source**: Should GDS auto-download CDTB hashes from CommunityDragon, or require manual placement? Auto-download adds a network dependency but improves UX.

### Technical Risks

| Risk | Mitigation |
|------|------------|
| Startup time with 180K+ bin objects | Index caching (decision #1); progress reporting; lazy loading of bins only when queried |
| Memory usage of full property registry | Consider mmap-backed storage or on-demand bin parsing (trade CPU for memory) |
| PropertyValueEnum serde format may be verbose for deeply nested types | ApiPropertyValue translation layer; consider custom serializer for containers/maps |
| Hash database incompleteness | Graceful fallback to hex display; allow user-contributed hashes; compute missing hashes from context |
| Game version updates changing bin/WAD formats | league-toolkit already handles version ranges; GDS rebuilds index on game update |

---

## Summary

| Phase | Scope | Key Outcome |
|-------|-------|-------------|
| **Phase 1** | Core library | `Workspace`, `PropertyRegistry`, `HashDatabase`, `WadIndex` |
| **Phase 2** | Read-only server | HTTP API for querying game data with Swagger docs |
| **Phase 3** | Layer system | Non-destructive delta-based property modifications |
| **Phase 4** | Build pipeline | Produce `.wad.client` mod files from layers |
| **Phase 5** | CLI tool | Command-line interface for all operations |
| **Phase 6** | Advanced | Schema inference, WebSocket, ritobin import/export, file watcher |

Phase 1 + 2 alone provide significant value as a **game data exploration tool**. Phase 3 + 4 complete the **modding workflow**. Phase 5 + 6 add **polish and productivity**.
