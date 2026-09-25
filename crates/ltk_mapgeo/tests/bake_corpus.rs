//! Re-bakes every shipped scene graph, ignored unless you point it at an installed client.
//!
//! ```text
//! LTK_LOL_GAME_DIR="C:/Riot Games/League of Legends/Game" \
//!     cargo test -p ltk_mapgeo --test bake_corpus --release -- --ignored --nocapture
//! ```
//!
//! For every `.mapgeo` in the map WADs, the selection read back from its scene graphs has to
//! bake into the same scene graphs, bit for bit. Files before version 15 hold exactly one grid,
//! which is the empty layout when nothing is baked.

use std::{
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
};

use ltk_mapgeo::{
    BucketedGeometry, EnvironmentAsset, GridLayout, SceneGraphKey, SceneGraphSelection, MAGIC,
};
use ltk_wad::Wad;

mod common;
use common::differences;

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

/// A face the shipped grids hold that [`EnvironmentAsset::default_face_mask`] leaves out.
fn default_rule_misses(
    asset: &EnvironmentAsset,
    selection: &SceneGraphSelection,
) -> Option<String> {
    let min_top_y = asset.lowest_selected_top_y(selection).unwrap()?;
    for (mesh, shipped) in selection.meshes().iter().enumerate() {
        if shipped.count() == 0 {
            continue;
        }
        let default = asset.default_face_mask(mesh, min_top_y).unwrap();
        let missed = shipped
            .iter()
            .zip(default.iter())
            .filter(|&(s, d)| s && !d)
            .count();
        if missed > 0 {
            return Some(format!("default rule misses {missed} faces of mesh {mesh}"));
        }
    }
    None
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn shipped_scene_graphs_rebake_exactly() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };
    let mut wads = Vec::new();
    map_wads(
        &Path::new(&game_dir).join("DATA/FINAL/Maps/Shipping"),
        &mut wads,
    );
    assert!(!wads.is_empty(), "no map WADs under {game_dir}");

    let (mut files, mut graphs, mut unreadable) = (0, 0, Vec::new());
    let mut failures = Vec::new();

    for wad_path in &wads {
        let mut wad =
            Wad::mount(File::open(wad_path).expect("the wad opens")).expect("the wad mounts");
        let chunks: Vec<_> = wad.chunks().as_slice().to_vec();
        for chunk in &chunks {
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if !data.starts_with(MAGIC) {
                continue;
            }
            let name = format!(
                "{} {:016x}",
                wad_path.file_name().unwrap().to_string_lossy(),
                chunk.path_hash
            );
            let asset = match EnvironmentAsset::from_reader(&mut Cursor::new(&data[..])) {
                Ok(asset) => asset,
                Err(e) => {
                    unreadable.push(format!("{name}: {e}"));
                    continue;
                }
            };
            files += 1;

            let selection = match asset.scene_graph_selection() {
                Ok(selection) => selection,
                Err(e) => {
                    failures.push(format!("{name}: {e}"));
                    continue;
                }
            };
            if let Some(missed) = default_rule_misses(&asset, &selection) {
                failures.push(format!("{name}: {missed}"));
            }
            let baked = asset.bake_scene_graphs(&selection);
            let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
            let baked = match baked {
                Ok(baked) if baked.is_empty() && version < 15 => {
                    vec![
                        BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(1), &[])
                            .unwrap(),
                    ]
                }
                Ok(baked) => baked,
                Err(e) => {
                    failures.push(format!("{name}: {e}"));
                    continue;
                }
            };
            if baked.len() != asset.scene_graphs().len() {
                failures.push(format!(
                    "{name}: baked {} scene graphs, shipped {}",
                    baked.len(),
                    asset.scene_graphs().len()
                ));
                continue;
            }
            for (i, (b, s)) in baked.iter().zip(asset.scene_graphs()).enumerate() {
                graphs += 1;
                let diff = differences(b, s);
                if !diff.is_empty() {
                    failures.push(format!("{name} graph {i}: {}", diff.join(", ")));
                }
            }
        }
    }

    println!("{files} files, {graphs} scene graphs re-baked");
    for u in &unreadable {
        println!("skipped, does not read: {u}");
    }
    assert!(files > 0, "no .mapgeo read");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
