//! Writes every shipped `.mapgeo`, ignored unless you point it at an installed client.
//!
//! ```text
//! LTK_LOL_GAME_DIR="C:/Riot Games/League of Legends/Game" \
//!     cargo test -p ltk_mapgeo --test write_corpus --release -- --ignored --nocapture
//! ```
//!
//! Version 18 files must write back byte for byte. Older files are written as version 18 and
//! must read back with the same buffers, meshes and scene graphs.

use std::{
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
};

use ltk_mapgeo::{EnvironmentAsset, MAGIC, WRITE_VERSION};
use ltk_wad::Wad;

mod common;
use common::asset_differences;

const GAME_DIR: &str = "LTK_LOL_GAME_DIR";

/// Every `Map*.wad.client` under `root`.
fn map_wads(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() {
            map_wads(&path, found);
        } else if name.starts_with("Map") && name.ends_with(".wad.client") {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn shipped_files_write_back() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };
    let mut wads = Vec::new();
    map_wads(
        &Path::new(&game_dir).join("DATA/FINAL/Maps/Shipping"),
        &mut wads,
    );
    assert!(!wads.is_empty(), "no map WADs under {game_dir}");

    let (mut exact, mut converted, mut unreadable, mut failures) = (0, 0, 0, Vec::new());
    for wad_path in &wads {
        let mut wad = Wad::mount(File::open(wad_path).unwrap()).unwrap();
        let chunks: Vec<_> = wad.chunks().as_slice().to_vec();
        for chunk in &chunks {
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if data.len() < 8 || &data[..4] != MAGIC {
                continue;
            }
            let name = format!(
                "{} {:016x}",
                wad_path.file_name().unwrap().to_string_lossy(),
                chunk.path_hash().0
            );
            let Ok(asset) = EnvironmentAsset::from_reader(&mut Cursor::new(&data[..])) else {
                unreadable += 1;
                continue;
            };
            let mut written = Vec::new();
            if let Err(e) = asset.to_writer(&mut written) {
                failures.push(format!("{name}: {e}"));
                continue;
            }
            let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
            if version == WRITE_VERSION {
                if written[..] == data[..] {
                    exact += 1;
                } else {
                    let at = written
                        .iter()
                        .zip(data.iter())
                        .position(|(a, b)| a != b)
                        .unwrap_or(written.len().min(data.len()));
                    let count = written
                        .iter()
                        .zip(data.iter())
                        .filter(|(a, b)| a != b)
                        .count();
                    let lo = at.saturating_sub(8);
                    failures.push(format!(
                        "{name}: first difference at {at:#x}, {count} bytes differ
  written {:02x?}
  shipped {:02x?}",
                        &written[lo..at + 8],
                        &data[lo..at + 8]
                    ));
                }
                continue;
            }
            let back = EnvironmentAsset::from_reader(&mut Cursor::new(&written[..]));
            match back {
                Ok(back) => {
                    let diffs = asset_differences(&asset, &back);
                    if diffs.is_empty() {
                        converted += 1;
                    } else {
                        failures.push(format!(
                            "{name} (v{version}): {}",
                            diffs[..diffs.len().min(4)].join("; ")
                        ));
                    }
                }
                Err(e) => failures.push(format!("{name} (v{version}): reads back with {e}")),
            }
        }
    }
    println!("{exact} v{WRITE_VERSION} files written byte for byte, {converted} older files converted, {unreadable} unreadable");
    for f in &failures {
        println!("{f}");
    }
    assert!(failures.is_empty(), "{} files failed", failures.len());
    assert!(exact > 0, "no v{WRITE_VERSION} files found");
}
