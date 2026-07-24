//! Reading and writing the League of Legends Lua script manifest,
//! `DATA/all_lua_files.manifest` (found in `Scripts.wad.client`).
//!
//! The manifest used to be a plain newline-separated list of paths. Since the
//! `RLM0` revision it stores *bare script names* plus enough metadata for the
//! game to rebuild each path from a table of hardcoded directories - so getting
//! paths back out means replaying that logic.
//!
//! # How a path is rebuilt
//!
//! - **Shared scripts** get an exact directory. Each has a packed
//!   [`ScriptEntry`] - `XXH3_64(lowercase(name)) << 5 | dir_index` - whose low 5
//!   bits index the 16 hardcoded [`SharedScriptDir`]s:
//!
//!   ```text
//!   DATA/Shared/Spells/ + Foo + .luabin64
//!   ```
//!
//!   The extension is still a guess: `.luabin64` always exists, `.preload` only
//!   sometimes.
//!
//! - **Character scripts** are not. The game probes the four
//!   [`CHARACTER_SUBDIRS`] under `DATA/Characters/<character>/` and takes the
//!   first file that exists, so [`LuaManifest::character_paths`] yields all four
//!   candidates per script - filter them against the WAD's path hashes
//!   (`xxh64` of the lowercased path) to find the real one.
//!
//! - **Map-scoped scripts** have no entry at all - which is why the shared list
//!   is longer than the entry table. They live under `LEVELS/Map<N>/Scripts/` or
//!   `LEVELS/Map<N>/Scripts/Mutators/`, and neither the map id nor the
//!   `Mutators/` hop is in the file. See [`LuaManifest::map_scoped_shared`] and
//!   [`LuaManifest::map_paths`].
//!
//! Only the first of those is a lookup the *format* performs. The rest is the
//! game's own probing logic replayed here, so anything this crate calls a
//! candidate should be checked against a WAD's path hashes (`xxh64` of the
//! lowercased path) before being treated as a file.
//!
//! - A few names have an entry carrying the [`NO_SHARED_DIR`] sentinel rather
//!   than a directory index: the manifest knows the name but records no home for
//!   it, and shipped builds pack no file for them at all. See
//!   [`LuaManifest::unplaced_shared`].
//!
//! # Example
//!
//! ```no_run
//! use ltk_lua::LuaManifest;
//!
//! let mut file = std::fs::File::open("all_lua_files.manifest")?;
//! let manifest = LuaManifest::from_reader(&mut file)?;
//!
//! // exact
//! for path in manifest.shared_paths() {
//!     println!("{path}");
//! }
//! // candidates - verify against a WAD before trusting them
//! for path in manifest.character_paths() {
//!     println!("{path}?");
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Format
//!
//! All integers little-endian; `str` is a `u32` byte length followed by that
//! many bytes, *not* NUL-terminated.
//!
//! ```text
//! char   magic[4]                  // "0MLR" ("RLM0" as a LE u32)
//! u32    characterCount
//! repeat characterCount:
//!     str    characterName         // e.g. "aatrox"
//!     u32    scriptCount
//!     repeat scriptCount: str scriptName
//! u32    sharedCount
//! repeat sharedCount: str sharedName
//! u32    entryCount
//! repeat entryCount: u64 entry     // (XXH3_64(lower(name)) << 5) | dirIndex
//! ```
//!
//! The character list and the entry table are both sorted - the game
//! binary-searches them.

mod dir;
mod entry;
mod error;
mod manifest;

pub use dir::*;
pub use entry::*;
pub use error::*;
pub use manifest::*;
