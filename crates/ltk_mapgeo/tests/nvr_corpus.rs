//! Lifts every `.nvr` under a directory, ignored unless you point it at one.
//!
//! ```text
//! LTK_NVR_DIR="C:/path/to/old/client/LEVELS" \
//!     cargo test -p ltk_mapgeo --test nvr_corpus --release -- --ignored --nocapture
//! ```
//!
//! Each file must lift, write as version 18, and read back with the same geometry.

use std::{
    fs::File,
    io::{BufReader, Cursor},
    path::{Path, PathBuf},
};

use ltk_mapgeo::{
    nvr::{MaterialFlags, SimpleEnvironment},
    EnvironmentAsset,
};

const NVR_DIR: &str = "LTK_NVR_DIR";

/// Every `.nvr` under `root`.
fn nvr_files(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            nvr_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("nvr"))
        {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "needs .nvr files; set LTK_NVR_DIR"]
fn nvr_files_lift_and_write() {
    let Ok(dir) = std::env::var(NVR_DIR) else {
        panic!("set {NVR_DIR} to a directory holding .nvr files");
    };
    let mut files = Vec::new();
    nvr_files(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "no .nvr files under {dir}");

    for path in &files {
        let nvr = SimpleEnvironment::from_reader(&mut BufReader::new(File::open(path).unwrap()))
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let asset = nvr
            .to_environment_asset(|m| m.name().to_owned())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

        let mut bytes = Vec::new();
        asset.to_writer(&mut bytes).unwrap();
        let back = EnvironmentAsset::from_reader(&mut Cursor::new(&bytes)).unwrap();

        assert_eq!(back.meshes().len(), nvr.meshes().len());
        for (mesh, lifted) in nvr.meshes().iter().zip(back.meshes()) {
            let vertices = &back.vertex_buffers()[lifted.vertex_buffer_ids()[0]];
            assert_eq!(vertices.as_bytes(), nvr.detailed_vertices(mesh));
            assert_eq!(lifted.index_count() as usize, mesh.detailed().index_count());
        }

        let ground_faces: usize = nvr
            .meshes()
            .iter()
            .filter(|m| nvr.material_of(m).flags().contains(MaterialFlags::GROUND))
            .map(|m| m.detailed().index_count() / 3)
            .sum();
        let baked_faces: usize = back.scene_graphs()[0]
            .buckets()
            .iter()
            .map(|b| b.total_face_count() as usize)
            .sum();
        assert_eq!(baked_faces, ground_faces, "{}", path.display());

        println!(
            "{}: version {:?}, {} meshes, {} ground faces, {} bytes",
            path.display(),
            nvr.version(),
            nvr.meshes().len(),
            ground_faces,
            bytes.len()
        );
    }
}
