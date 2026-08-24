//! A corpus check over an installed client, ignored unless you point it at one.
//!
//! ```text
//! LTK_LOL_GAME_DIR="C:/Riot Games/League of Legends/Game" \
//!     cargo test -p ltk_meta --test corpus -- --ignored --nocapture
//! ```
//!
//! It is the permanent replacement for the scratch tooling the design was written from: every
//! `PTCH` chunk in the install has to read, re-write byte for byte, and have every one of its
//! records resolve against the real objects those records name.

use std::{
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::Cursor,
    path::{Path, PathBuf},
};

use ltk_hash::BinHash;
use ltk_meta::{
    path::{PatchError, ResolveErrorKind},
    Bin, BinKind, BinObject, BinOverride,
};
use ltk_wad::Wad;

const GAME_DIR: &str = "LTK_LOL_GAME_DIR";

#[derive(Default)]
struct Counts {
    wads: usize,
    prop_chunks: usize,
    patch_chunks: usize,
    rewritten: usize,
    records: usize,
    objects: usize,
    deletions: usize,
    applied: usize,
    inserted: usize,
    missing_object: usize,
    missing_property: usize,
    /// Anything else, which section 2.1 of the design measured as zero.
    unexpected: Vec<String>,
}

impl fmt::Display for Counts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} wad archives", self.wads)?;
        writeln!(
            f,
            "{} PROP chunks read, of the archives that carry a patch",
            self.prop_chunks
        )?;
        writeln!(
            f,
            "{} PTCH chunks read, {} re-written byte for byte",
            self.patch_chunks, self.rewritten
        )?;
        writeln!(
            f,
            "{} records / {} whole objects / {} deletions",
            self.records, self.objects, self.deletions
        )?;
        writeln!(
            f,
            "{} records resolve ({} of them create the leaf)",
            self.applied, self.inserted
        )?;
        writeln!(
            f,
            "{} skipped: no such object, {} skipped: an intermediate property is absent",
            self.missing_object, self.missing_property
        )?;
        write!(f, "{} skipped for any other reason", self.unexpected.len())
    }
}

/// Every `.wad.client` under `root`.
fn wad_paths(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            wad_paths(&path, found);
        } else if path.to_string_lossy().ends_with(".wad.client") {
            found.push(path);
        }
    }
}

/// Reads every `PTCH` chunk, checking that it survives a round trip through the writer.
fn patches(wad_path: &Path, counts: &mut Counts) -> Vec<BinOverride> {
    let source = File::open(wad_path).expect("the wad opens");
    let mut wad = Wad::mount(source).expect("the wad mounts");
    let chunks: Vec<_> = wad.chunks().as_slice().to_vec();

    let mut patches = Vec::new();
    for chunk in &chunks {
        let Ok(data) = wad.load_chunk_decompressed(chunk) else {
            continue;
        };
        if BinKind::identify_from_bytes(&data) != Some(BinKind::Override) {
            continue;
        }

        counts.patch_chunks += 1;
        let patch_bin = BinOverride::from_reader(&mut Cursor::new(&data)).unwrap_or_else(|e| {
            panic!(
                "{}: chunk {:016x} did not read: {e}",
                wad_path.display(),
                chunk.path_hash
            )
        });

        let mut written = Cursor::new(Vec::new());
        patch_bin.to_writer(&mut written).expect("the patch writes");
        if written.into_inner() == data.as_ref() {
            counts.rewritten += 1;
        }

        counts.records += patch_bin.patches.len();
        counts.objects += patch_bin.objects.len();
        counts.deletions += patch_bin.deleted.len();
        patches.push(patch_bin);
    }

    patches
}

/// Reads every `PROP` chunk, keeping the objects the patches actually address.
///
/// A bin object's path hash is the hash of its asset path, so collecting by hash lands each record
/// on the object it names without needing to know which file that object lives in.
fn wanted_objects(
    wad_path: &Path,
    wanted: &HashSet<BinHash>,
    counts: &mut Counts,
) -> HashMap<BinHash, BinObject> {
    let source = File::open(wad_path).expect("the wad opens");
    let mut wad = Wad::mount(source).expect("the wad mounts");
    let chunks: Vec<_> = wad.chunks().as_slice().to_vec();

    let mut found = HashMap::new();
    for chunk in &chunks {
        let Ok(data) = wad.load_chunk_decompressed(chunk) else {
            continue;
        };
        if BinKind::identify_from_bytes(&data) != Some(BinKind::Prop) {
            continue;
        }

        counts.prop_chunks += 1;
        let bin = Bin::from_reader(&mut Cursor::new(&data)).unwrap_or_else(|e| {
            panic!(
                "{}: chunk {:016x} did not read: {e}",
                wad_path.display(),
                chunk.path_hash
            )
        });

        for (object_hash, object) in bin.objects {
            if wanted.contains(&object_hash) {
                found.insert(object_hash, object);
            }
        }
    }

    found
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn every_shipped_patch_reads_rewrites_and_resolves() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };

    let mut wad_files = Vec::new();
    wad_paths(Path::new(&game_dir), &mut wad_files);
    wad_files.sort();
    assert!(!wad_files.is_empty(), "no .wad.client under {game_dir}");

    let mut counts = Counts::default();
    for wad_path in &wad_files {
        counts.wads += 1;

        let patches = patches(wad_path, &mut counts);
        if patches.is_empty() {
            continue;
        }

        let wanted: HashSet<BinHash> = patches
            .iter()
            .flat_map(|patch_bin| patch_bin.patches.iter().map(|patch| patch.object_hash))
            .collect();
        let objects = wanted_objects(wad_path, &wanted, &mut counts);
        let base = Bin::new(objects.into_values(), std::iter::empty::<&str>());

        for patch_bin in &patches {
            let report = patch_bin.check(&base);
            counts.applied += report.applied;
            counts.inserted += report.inserted;

            for skipped in report.skipped {
                match skipped.error {
                    PatchError::Resolve(error) => match error.kind() {
                        ResolveErrorKind::MissingObject(_) => counts.missing_object += 1,
                        ResolveErrorKind::MissingProperty(_) => counts.missing_property += 1,
                        _ => counts.unexpected.push(skipped.to_string()),
                    },
                    PatchError::TypeMismatch { .. } => counts.unexpected.push(skipped.to_string()),
                }
            }
        }
    }

    println!("{counts}");

    assert!(counts.patch_chunks > 0, "no PTCH chunks in {game_dir}");
    assert_eq!(
        counts.rewritten, counts.patch_chunks,
        "some patches did not survive a round trip through the writer"
    );
    // Section 2.1 of the design measured every one of these as zero across the corpus: a shipped
    // record never mismatches a type, subscripts something unsubscriptable, runs off the end of a
    // container or walks into a null pointer. Only stale paths are skipped.
    assert!(
        counts.unexpected.is_empty(),
        "records skipped for a reason section 2.1 measured as zero:\n{}",
        counts.unexpected.join("\n")
    );
}
