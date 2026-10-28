//! Shared helpers of the `ltk_wwise` integration tests.
//!
//! The corpus helpers read `LTK_LOL_GAME_DIR`, the `Game` directory of an installed client.

#![expect(dead_code, reason = "each test binary uses a different subset")]

use std::{
    fs::File,
    path::{Path, PathBuf},
};

use ltk_wad::{decompress_prefix, Wad};

/// The environment variable that names the `Game` directory of an installed client.
pub const GAME_DIR: &str = "LTK_LOL_GAME_DIR";

/// The raw bytes read to sniff the magic of a chunk.
const PREFIX_LEN: usize = 132 * 1024;

/// Returns the bytes of the fixture `name` in `tests/fixtures`.
pub fn fixture(name: &str) -> Vec<u8> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

/// One shipped chunk.
pub struct CorpusChunk {
    pub wad: PathBuf,
    pub path_hash: u64,
    pub data: Box<[u8]>,
}

/// Calls `visit` with every chunk of every WAD under `DATA/FINAL` whose bytes start with `magic`.
///
/// # Panics
///
/// Panics when `LTK_LOL_GAME_DIR` is not set or names a directory without WADs.
pub fn for_each_chunk(magic: &[u8], mut visit: impl FnMut(&CorpusChunk)) {
    let game_dir = std::env::var(GAME_DIR)
        .unwrap_or_else(|_| panic!("set {GAME_DIR} to the client's Game directory"));
    let mut wads = Vec::new();
    find_wads(&Path::new(&game_dir).join("DATA/FINAL"), &mut wads);
    assert!(!wads.is_empty(), "no WADs under {game_dir}");

    for wad_path in wads {
        let Ok(file) = File::open(&wad_path) else {
            continue;
        };
        let Ok(mut wad) = Wad::mount(file) else {
            continue;
        };
        let chunks = wad.chunks().as_slice().to_vec();
        for chunk in &chunks {
            // A zstd block holds at most 128 KiB. The frame header and the first block fit in the
            // prefix. A prefix that does not decode falls through to the full chunk.
            let Ok(raw) = wad.load_chunk_raw_prefix(chunk, PREFIX_LEN) else {
                continue;
            };
            if let Ok(prefix) = decompress_prefix(&raw, chunk.compression_type(), magic.len()) {
                if !prefix.starts_with(magic) {
                    continue;
                }
            }
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if data.starts_with(magic) {
                visit(&CorpusChunk {
                    wad: wad_path.clone(),
                    path_hash: chunk.path_hash().0,
                    data,
                });
            }
        }
    }
}

fn find_wads(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_wads(&path, found);
        } else if path.to_string_lossy().ends_with(".wad.client") {
            found.push(path);
        }
    }
}
