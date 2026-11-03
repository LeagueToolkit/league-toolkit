//! Reads and writes every `.aimesh_ngrid`, `.ngrid_overlay` and `.rg_overlay` file in a directory.
//! The test is ignored unless it is run with `--ignored`, and it fails if `LTK_NAVGRID_DIR` is not
//! set.
//!
//! ```text
//! wadtools extract -i Map11.wad.client -o navgrid -x "navgrid|ngrid|rg_overlay"
//! LTK_NAVGRID_DIR="navgrid" \
//!     cargo test -p ltk_navgrid --test corpus --release -- --ignored --nocapture
//! ```
//!
//! Every file must read to its last byte and write back byte for byte. Every grid cell must store
//! its own coordinates as its locator.

use std::path::{Path, PathBuf};

use ltk_navgrid::{NavGrid, NavGridOverlay, RegionOverlay};

const NAVGRID_DIR: &str = "LTK_NAVGRID_DIR";
const EXTENSIONS: [&str; 3] = ["aimesh_ngrid", "ngrid_overlay", "rg_overlay"];

/// Appends the path of every navigation grid file in `root` and its subdirectories to `found`.
fn navgrid_files(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            navgrid_files(&path, found);
        } else if path
            .extension()
            .is_some_and(|e| EXTENSIONS.iter().any(|ext| e.eq_ignore_ascii_case(ext)))
        {
            found.push(path);
        }
    }
}

/// Reads `bytes` as the format of the extension of `path` and returns the written bytes.
/// Returns an error message if the read or the write fails or leaves bytes unread.
fn rewrite(path: &Path, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let ext = path
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    let mut written = Vec::new();
    let reader = &mut &bytes[..];
    match ext.as_str() {
        "aimesh_ngrid" => {
            let grid = NavGrid::from_reader(reader).map_err(|e| e.to_string())?;
            // Shipped records store the cell's own coordinates. A field offset error breaks this.
            let width = grid.x_cell_count() as usize;
            for (i, cell) in grid.cells().iter().enumerate() {
                if cell.locator != [(i % width) as i16, (i / width) as i16]
                    || cell.session_id != u32::MAX
                {
                    return Err(format!("cell {i} has record {cell:?}"));
                }
            }
            grid.to_writer(&mut written).map_err(|e| e.to_string())?;
        }
        "ngrid_overlay" => NavGridOverlay::from_reader(reader)
            .map_err(|e| e.to_string())?
            .to_writer(&mut written)
            .map_err(|e| e.to_string())?,
        _ => RegionOverlay::from_reader(reader)
            .map_err(|e| e.to_string())?
            .to_writer(&mut written)
            .map_err(|e| e.to_string())?,
    }
    if !reader.is_empty() {
        return Err(format!("{} bytes not read", reader.len()));
    }
    Ok(written)
}

#[test]
#[ignore = "The test needs extracted navigation grid files. Set LTK_NAVGRID_DIR"]
fn shipped_files_round_trip() {
    let Ok(dir) = std::env::var(NAVGRID_DIR) else {
        panic!(
            "{NAVGRID_DIR} is not set. Set it to a directory that contains navigation grid files"
        );
    };
    let mut files = Vec::new();
    navgrid_files(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "{dir} contains no navigation grid files");

    let mut counts = [0usize; EXTENSIONS.len()];
    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        match rewrite(path, &bytes) {
            Ok(written) if written == bytes => {
                let ext = path
                    .extension()
                    .unwrap()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                counts[EXTENSIONS.iter().position(|e| *e == ext).unwrap()] += 1;
            }
            Ok(_) => failures.push(format!("{}: written bytes differ", path.display())),
            Err(e) => failures.push(format!("{}: {e}", path.display())),
        }
    }
    for (ext, count) in EXTENSIONS.iter().zip(counts) {
        println!("{count} .{ext} files round-tripped");
    }
    for failure in &failures {
        println!("FAIL {failure}");
    }
    assert!(failures.is_empty(), "{} files failed", failures.len());
}
