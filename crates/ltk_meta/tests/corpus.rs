//! A corpus check over an installed client, ignored unless you point it at one.
//!
//! ```text
//! LTK_LOL_GAME_DIR="C:/Riot Games/League of Legends/Game" \
//!     cargo test -p ltk_meta --test corpus -- --ignored --nocapture
//! ```
//!
//! It is the permanent replacement for the scratch tooling the design was written from: every
//! `PTCH` chunk in the install has to read, re-write byte for byte, and have every one of its
//! records resolve against the real objects those records name. Every `PROP` chunk additionally
//! has to stream: mounting and sweeping it harvests the same object set the eager parse holds,
//! every object views cleanly, and every property's wire shape and decoded value agree with what
//! the eager parse holds for it.

use std::{
    cell::Cell,
    collections::{HashMap, HashSet},
    fmt,
    fs::File,
    io::{self, Cursor, Read, Seek},
    path::{Path, PathBuf},
    rc::Rc,
    time::{Duration, Instant},
};

use ltk_hash::BinHash;
use ltk_meta::{
    path::{PatchError, PropertyPath, ResolveErrorKind, Subscript, ValueShape},
    traits::PropertyExt as _,
    walk::{Node, NodeRefMut, TreeValue, Visit, Visitor, VisitorMut},
    Bin, BinDelta, BinKind, BinObject, BinOverride, BinStream, Error, Lift, PropertyValueEnum,
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
                    // `PatchError` is non-exhaustive; any kind added later is unexpected too.
                    _ => counts.unexpected.push(skipped.to_string()),
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

#[derive(Default)]
struct DiffCounts {
    patches: usize,
    records: usize,
    objects: usize,
    map_inserts: usize,
    nameless: usize,
    mismatches: usize,
    /// Diff records carrying a whole container a shipped record indexes into.
    containers: usize,
    /// A diff record that lies outside every record of the patch it came from.
    outside: Vec<String>,
}

impl fmt::Display for DiffCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} patches diffed back: {} records, {} whole objects",
            self.patches, self.records, self.objects
        )?;
        writeln!(
            f,
            "lifted: {} map inserts, {} nameless, {} mismatches",
            self.map_inserts, self.nameless, self.mismatches
        )?;
        writeln!(
            f,
            "{} records carry a whole container a shipped record indexes into",
            self.containers
        )?;
        write!(f, "{} records outside the patch", self.outside.len())
    }
}

/// A path's segments as `(name hash, subscript)`, which is what the client resolves by.
fn resolved_by(path: &PropertyPath) -> Vec<(BinHash, Option<Subscript<'_>>)> {
    path.segments()
        .map(|segment| (segment.name_hash(), segment.subscript.clone()))
        .collect()
}

/// Whether `original` subscripts the container `list` names: every segment of `list` but its last
/// matches, and the next segment of `original` names the same property.
fn indexes_into(
    original: &[(BinHash, Option<Subscript<'_>>)],
    list: &[(BinHash, Option<Subscript<'_>>)],
) -> bool {
    let Some(((name, subscript), parents)) = list.split_last() else {
        return false;
    };
    subscript.is_none()
        && original.len() > parents.len()
        && original[..parents.len()] == *parents
        && original[parents.len()].0 == *name
}

/// `ptch-property-patches.md` section 16: every shipped patch, applied to the objects it names
/// and diffed back, is a patch that applies as the merge does and writes nothing outside the
/// records it came from, but for the whole container one of them indexes into.
#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn every_shipped_patch_diffs_back_inside_its_records() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };

    let mut wad_files = Vec::new();
    wad_paths(Path::new(&game_dir), &mut wad_files);
    wad_files.sort();

    let mut ignored = Counts::default();
    let mut counts = DiffCounts::default();
    for wad_path in &wad_files {
        let patches = patches(wad_path, &mut ignored);
        if patches.is_empty() {
            continue;
        }
        let wanted: HashSet<BinHash> = patches
            .iter()
            .flat_map(|patch_bin| patch_bin.patches.iter().map(|patch| patch.object_hash))
            .collect();
        let objects = wanted_objects(wad_path, &wanted, &mut ignored);
        let base = Bin::new(objects.into_values(), std::iter::empty::<&str>());

        for patch_bin in &patches {
            counts.patches += 1;
            let mut edited = base.clone();
            patch_bin.clone().apply(&mut edited);
            let names: HashMap<BinHash, String> = patch_bin
                .patches
                .iter()
                .flat_map(|record| record.path.segments())
                .map(|segment| (segment.name_hash(), segment.name.to_owned()))
                .collect();

            let (diff, report) = base.diff(&edited, &names);
            counts.records += report.records;
            counts.objects += report.objects.len();
            for lift in &report.lifted {
                match lift {
                    Lift::MapInsert { .. } => counts.map_inserts += 1,
                    Lift::Nameless { .. } => counts.nameless += 1,
                    Lift::Mismatch { .. } => counts.mismatches += 1,
                    _ => {}
                }
            }
            for record in &diff.patches {
                let path = resolved_by(&record.path);
                let on_object = patch_bin
                    .patches
                    .iter()
                    .filter(|original| original.object_hash == record.object_hash);
                let inside = on_object
                    .clone()
                    .any(|original| path.starts_with(&resolved_by(&original.path)));
                // A container replaces whole (D22): a record inside one element is the whole list,
                // named by the shipped path's segment that subscripts it.
                let container = matches!(
                    record.value,
                    PropertyValueEnum::Container(_) | PropertyValueEnum::UnorderedContainer(_)
                ) && on_object
                    .clone()
                    .any(|original| indexes_into(&resolved_by(&original.path), &path));
                if container {
                    counts.containers += 1;
                }
                if !inside && !container {
                    counts
                        .outside
                        .push(format!("{:08x} {}", record.object_hash, record.path));
                }
            }

            // A whole object is one the patch carries, or one a lift took at its root.
            for object in &report.objects {
                let carried = patch_bin.objects.contains_key(object)
                    || report
                        .lifted
                        .iter()
                        .any(|lift| lift.object_hash() == *object);
                if !carried {
                    counts.outside.push(format!("{object:08x} whole"));
                }
            }

            let mut applied = base.clone();
            let applied_report = diff.apply(&mut applied);
            assert!(
                applied_report.is_clean(),
                "{}: {:?}",
                wad_path.display(),
                applied_report.skipped
            );
            let mut merged = base.clone();
            merged.merge(&edited);
            assert_eq!(applied.objects, merged.objects, "{}", wad_path.display());
        }
    }

    println!("{counts}");
    assert!(counts.patches > 0, "no PTCH chunks in {game_dir}");
    assert!(
        counts.outside.is_empty(),
        "diff records outside the patch they came from:\n{}",
        counts.outside.join("\n")
    );
}

/// Counts the reads that reach the wrapped source, to catch a sweep that re-harvests.
struct CountingReads<R> {
    inner: R,
    reads: Rc<Cell<usize>>,
}

impl<R> CountingReads<R> {
    fn new(inner: R) -> (Self, Rc<Cell<usize>>) {
        let reads = Rc::new(Cell::new(0));
        (
            Self {
                inner,
                reads: Rc::clone(&reads),
            },
            reads,
        )
    }
}

impl<R: Read> Read for CountingReads<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read(buf)
    }
}

impl<R: Seek> Seek for CountingReads<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }
}

#[derive(Default)]
struct StreamCounts {
    wads: usize,
    prop_chunks: usize,
    objects: usize,
    properties: usize,
    viewed: usize,
    batched: usize,
    legacy_chunks: usize,
}

impl fmt::Display for StreamCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} wad archives", self.wads)?;
        writeln!(f, "{} PROP chunks mounted and swept", self.prop_chunks)?;
        writeln!(
            f,
            "{} objects harvested / {} properties measured",
            self.objects, self.properties
        )?;
        writeln!(
            f,
            "{} properties viewed, shaped and decoded against the eager parse",
            self.viewed
        )?;
        writeln!(f, "{} objects opened through a batch", self.batched)?;
        write!(
            f,
            "{} chunks latched onto the legacy kind numbering",
            self.legacy_chunks
        )
    }
}

/// The streaming harvest against the eager parse, for one `PROP` chunk.
fn check_stream_parity(wad_path: &Path, data: &[u8], counts: &mut StreamCounts) {
    let context = || format!("{}", wad_path.display());

    let eager = Bin::from_reader(&mut Cursor::new(data))
        .unwrap_or_else(|e| panic!("{}: the eager parse failed: {e}", context()));

    let (source, reads) = CountingReads::new(Cursor::new(data));
    let mut stream = BinStream::mount(source)
        .unwrap_or_else(|e| panic!("{}: the stream did not mount: {e}", context()));

    // Header facts, free after mount.
    assert_eq!(stream.version(), eager.version, "{}", context());
    assert_eq!(stream.dependencies(), eager.dependencies, "{}", context());
    assert_eq!(
        stream.class_hashes(),
        eager
            .objects
            .values()
            .map(|o| o.class_hash)
            .collect::<Vec<_>>(),
        "{}",
        context()
    );

    // The harvest sweep sees the object set the eager parse holds, in file order.
    let entries: Vec<_> = stream
        .entries()
        .collect::<Result<_, _>>()
        .unwrap_or_else(|e| panic!("{}: the sweep failed: {e}", context()));
    assert_eq!(
        entries
            .iter()
            .map(|e| (e.path_hash, e.class_hash))
            .collect::<Vec<_>>(),
        eager
            .objects
            .values()
            .map(|o| (o.path_hash, o.class_hash))
            .collect::<Vec<_>>(),
        "{}",
        context()
    );

    // Every declared object size agrees with `PropertyExt::size` over the parsed values.
    // The wire-core unit tests pin skip distances to `size` for every kind on constructed
    // values; this closes the loop by pinning `size` to the shipped bytes.
    for (entry, object) in entries.iter().zip(eager.objects.values()) {
        let measured: usize = 6 + object
            .properties
            .values()
            .map(|p| p.size(true))
            .sum::<usize>();
        assert_eq!(
            u64::from(entry.size),
            measured as u64,
            "{}: object {:08x} declares a size PropertyExt::size disagrees with",
            context(),
            entry.path_hash
        );
        counts.properties += object.properties.len();
    }

    // The sweep populated the TOC; asking for it (or sweeping again) reads nothing more.
    let reads_after_sweep = reads.get();
    let toc = stream
        .toc()
        .unwrap_or_else(|e| panic!("{}: the TOC did not build: {e}", context()));
    assert_eq!(toc.entries(), entries, "{}", context());
    assert_eq!(
        reads.get(),
        reads_after_sweep,
        "{}: a second harvest pass ran",
        context()
    );

    // Random access lands on the same object the eager map holds.
    if let Some((&path_hash, object)) = eager.objects.first() {
        let mut streamed = stream
            .object(path_hash)
            .unwrap_or_else(|e| panic!("{}: object lookup failed: {e}", context()))
            .unwrap_or_else(|| panic!("{}: {:08x} is not in the TOC", context(), path_hash));
        assert_eq!(streamed.class_hash(), object.class_hash, "{}", context());
        assert_eq!(
            streamed.property_count().expect("the count reads") as usize,
            object.properties.len(),
            "{}",
            context()
        );
    }

    // Every object, viewed: the borrowed renderer and the owned one over the same bytes agree
    // with each other and with the eager parse, property for property.
    let mut cursor = stream.objects();
    while let Some(mut object) = cursor
        .next()
        .unwrap_or_else(|e| panic!("{}: the cursor failed: {e}", context()))
    {
        let path_hash = object.path_hash();
        let view = object
            .view()
            .unwrap_or_else(|e| panic!("{}: object {path_hash:08x} did not view: {e}", context()));

        let expected = &eager.objects[&path_hash];
        assert_eq!(
            view.property_count() as usize,
            expected.properties.len(),
            "{}: object {path_hash:08x}",
            context()
        );

        for (property, (name_hash, value)) in view.properties().zip(expected.properties.iter()) {
            let property = property
                .unwrap_or_else(|e| panic!("{}: object {path_hash:08x} property: {e}", context()));
            let where_ = || format!("{}: object {path_hash:08x} {name_hash:08x}", context());

            assert_eq!(property.name_hash(), *name_hash, "{}", where_());
            assert_eq!(property.kind(), value.kind(), "{}", where_());
            assert_eq!(property.raw().len(), value.size_no_header(), "{}", where_());
            assert_eq!(
                property
                    .shape()
                    .unwrap_or_else(|e| panic!("{}: the shape did not read: {e}", where_())),
                ValueShape::of(value),
                "{}",
                where_()
            );
            assert_eq!(
                &property
                    .value()
                    .unwrap_or_else(|e| panic!("{}: the value did not decode: {e}", where_())),
                value,
                "{}",
                where_()
            );
            counts.viewed += 1;
        }
    }

    if stream.numbering().is_legacy() {
        counts.legacy_chunks += 1;
    }

    // A batch opens the same objects as the per-hash lookups, in file order.
    let sample: Vec<BinHash> = eager.objects.keys().rev().take(8).copied().collect();
    let one_by_one: Vec<BinObject> = sample
        .iter()
        .map(|&hash| {
            stream
                .object(hash)
                .unwrap_or_else(|e| panic!("{}: object lookup failed: {e}", context()))
                .unwrap_or_else(|| panic!("{}: {hash:08x} is not in the TOC", context()))
                .read()
                .unwrap_or_else(|e| panic!("{}: {hash:08x} did not read: {e}", context()))
        })
        .collect();

    let mut in_file_order: Vec<BinObject> = Vec::with_capacity(sample.len());
    let mut batch = stream.objects_batch(sample.iter().copied());
    while let Some(mut object) = batch
        .next()
        .unwrap_or_else(|e| panic!("{}: the batch failed: {e}", context()))
    {
        in_file_order.push(
            object
                .read()
                .unwrap_or_else(|e| panic!("{}: a batched object did not read: {e}", context())),
        );
    }
    assert!(batch.missing().is_empty(), "{}", context());

    let mut expected = one_by_one;
    expected.sort_by_key(|object| eager.objects.get_index_of(&object.path_hash));
    assert_eq!(in_file_order, expected, "{}", context());
    counts.batched += in_file_order.len();

    counts.prop_chunks += 1;
    counts.objects += entries.len();
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn every_shipped_prop_streams_the_same_object_set() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };

    let mut wad_files = Vec::new();
    wad_paths(Path::new(&game_dir), &mut wad_files);
    wad_files.sort();
    assert!(!wad_files.is_empty(), "no .wad.client under {game_dir}");

    let mut counts = StreamCounts::default();
    for wad_path in &wad_files {
        counts.wads += 1;

        let source = File::open(wad_path).expect("the wad opens");
        let mut wad = Wad::mount(source).expect("the wad mounts");
        let chunks: Vec<_> = wad.chunks().as_slice().to_vec();

        for chunk in &chunks {
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if BinKind::identify_from_bytes(&data) != Some(BinKind::Prop) {
                continue;
            }
            check_stream_parity(wad_path, &data, &mut counts);
        }
    }

    println!("{counts}");
    assert!(counts.prop_chunks > 0, "no PROP chunks in {game_dir}");
}

/// `(object, class, trail)` per node, in visit order.
#[derive(Default)]
struct Visits(Vec<(u32, u32, String)>);

impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Visits {
    type Error = Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
        self.0.push((
            *node.object_hash(),
            *node.class_hash(),
            node.trail().to_string(),
        ));
        Ok(Visit::Continue)
    }
}

impl VisitorMut for Visits {
    type Error = Error;

    fn enter_node(&mut self, node: &mut NodeRefMut<'_>) -> Result<Visit, Error> {
        self.0.push((
            *node.object_hash(),
            *node.class_hash(),
            node.trail().to_string(),
        ));
        Ok(Visit::Continue)
    }
}

/// The nodes under `value`, by a recursion independent of the walk: every `Struct` and
/// `Embedded` with a non-zero class, wherever it sits.
fn count_nodes(value: &PropertyValueEnum) -> usize {
    use PropertyValueEnum as P;
    let count_struct = |s: &ltk_meta::property::values::Struct| {
        if *s.class_hash == 0 {
            0
        } else {
            1 + s.properties.values().map(count_nodes).sum::<usize>()
        }
    };
    match value {
        P::Struct(s) => count_struct(s),
        P::Embedded(e) => count_struct(&e.0),
        P::Container(c) => c.items().iter().map(count_nodes).sum(),
        P::UnorderedContainer(c) => c.0.items().iter().map(count_nodes).sum(),
        P::Optional(o) => o.value().map_or(0, count_nodes),
        P::Map(m) => m.entries().iter().map(|(_, v)| count_nodes(v)).sum(),
        _ => 0,
    }
}

#[derive(Default)]
struct WalkCounts {
    wads: usize,
    prop_chunks: usize,
    objects: usize,
    nodes: usize,
}

impl fmt::Display for WalkCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} wad archives", self.wads)?;
        writeln!(f, "{} PROP chunks walked three times", self.prop_chunks)?;
        writeln!(f, "{} objects, {} nodes", self.objects, self.nodes)
    }
}

/// The streaming walk against the owned walk and an independent count, for one `PROP` chunk.
fn check_walk_parity(wad_path: &Path, data: &[u8], counts: &mut WalkCounts) {
    let context = || format!("{}", wad_path.display());

    let eager = Bin::from_reader(&mut Cursor::new(data))
        .unwrap_or_else(|e| panic!("{}: the eager parse failed: {e}", context()));
    let mut owned = Visits::default();
    eager
        .walk(&mut owned)
        .unwrap_or_else(|e| panic!("{}: the owned walk failed: {e}", context()));

    let mut stream = BinStream::mount(Cursor::new(data))
        .unwrap_or_else(|e| panic!("{}: the stream did not mount: {e}", context()));
    let mut streamed = Visits::default();
    stream
        .walk(&mut streamed)
        .unwrap_or_else(|e: Error| panic!("{}: the streaming walk failed: {e}", context()));

    assert_eq!(owned.0, streamed.0, "{}", context());

    let mut edited = eager.clone();
    let mut mutable = Visits::default();
    edited
        .walk_mut(&mut mutable)
        .unwrap_or_else(|e| panic!("{}: the mutable walk failed: {e}", context()));
    assert_eq!(owned.0, mutable.0, "{}", context());
    assert_eq!(edited, eager, "{}", context());

    let expected: usize = eager
        .objects
        .values()
        .map(|object| 1 + object.properties.values().map(count_nodes).sum::<usize>())
        .sum();
    assert_eq!(owned.0.len(), expected, "{}", context());

    counts.prop_chunks += 1;
    counts.objects += eager.objects.len();
    counts.nodes += expected;
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn every_shipped_prop_walks_the_same_over_both_trees() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };

    let mut wad_files = Vec::new();
    wad_paths(Path::new(&game_dir), &mut wad_files);
    wad_files.sort();
    assert!(!wad_files.is_empty(), "no .wad.client under {game_dir}");

    let mut counts = WalkCounts::default();
    for wad_path in &wad_files {
        counts.wads += 1;

        let source = File::open(wad_path).expect("the wad opens");
        let mut wad = Wad::mount(source).expect("the wad mounts");
        let chunks: Vec<_> = wad.chunks().as_slice().to_vec();

        for chunk in &chunks {
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if BinKind::identify_from_bytes(&data) != Some(BinKind::Prop) {
                continue;
            }
            check_walk_parity(wad_path, &data, &mut counts);
        }
    }

    println!("{counts}");
    assert!(counts.prop_chunks > 0, "no PROP chunks in {game_dir}");
}

#[derive(Default)]
struct DeltaCounts {
    wads: usize,
    prop_chunks: usize,
    bytes: usize,
    /// Mount, read one object, write the delta replacing it.
    delta: Duration,
    /// Parse the whole chunk, write the whole tree.
    transcode: Duration,
}

impl fmt::Display for DeltaCounts {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} wad archives", self.wads)?;
        writeln!(
            f,
            "{} PROP chunks, {} bytes, rewritten byte for byte through an empty delta",
            self.prop_chunks, self.bytes
        )?;
        writeln!(f, "one-object delta write: {:?}", self.delta)?;
        write!(f, "whole-file transcode:   {:?}", self.transcode)
    }
}

/// The delta write-back against the input and the eager transcode, for one `PROP` chunk.
fn check_delta_rewrite(wad_path: &Path, data: &[u8], counts: &mut DeltaCounts) {
    let context = || format!("{}", wad_path.display());

    let mut stream = BinStream::mount(Cursor::new(data))
        .unwrap_or_else(|e| panic!("{}: the stream did not mount: {e}", context()));
    let mut out = Vec::with_capacity(data.len());
    stream
        .write_patched(&BinDelta::new(), &mut out)
        .unwrap_or_else(|e| panic!("{}: the empty delta failed: {e}", context()));
    assert!(
        out == data,
        "{}: an empty delta changed the bytes",
        context()
    );

    let middle = {
        let entries = stream.toc().expect("the TOC builds").entries();
        if entries.is_empty() {
            counts.prop_chunks += 1;
            return;
        }
        entries[entries.len() / 2].path_hash
    };

    let started = Instant::now();
    let mut stream = BinStream::mount(Cursor::new(data)).expect("the stream mounts");
    let object = stream
        .object(middle)
        .and_then(|object| object.expect("the object is in the TOC").read())
        .unwrap_or_else(|e| panic!("{}: the object did not read: {e}", context()));
    let mut delta = BinDelta::new();
    delta.replace(object);
    let mut patched = Vec::with_capacity(data.len());
    stream
        .write_patched(&delta, &mut patched)
        .unwrap_or_else(|e| panic!("{}: the delta failed: {e}", context()));
    counts.delta += started.elapsed();

    let started = Instant::now();
    let eager = Bin::from_reader(&mut Cursor::new(data)).expect("the eager parse succeeds");
    let mut transcoded = Cursor::new(Vec::with_capacity(data.len()));
    eager.to_writer(&mut transcoded).expect("the bin writes");
    counts.transcode += started.elapsed();

    let reread = Bin::from_reader(&mut Cursor::new(&patched))
        .unwrap_or_else(|e| panic!("{}: the delta output did not read: {e}", context()));
    assert_eq!(reread, eager, "{}", context());

    let ranges = |bytes: &[u8]| {
        let mut stream = BinStream::mount(Cursor::new(bytes)).expect("the stream mounts");
        stream
            .toc()
            .expect("the TOC builds")
            .entries()
            .iter()
            .map(|entry| (entry.path_hash, entry.byte_range()))
            .collect::<Vec<_>>()
    };
    for ((hash, before), (_, after)) in ranges(data).into_iter().zip(ranges(&patched)) {
        if hash != middle {
            let slice = |bytes: &[u8], range: std::ops::Range<u64>| {
                bytes[range.start as usize..range.end as usize].to_vec()
            };
            assert!(
                slice(data, before) == slice(&patched, after),
                "{}: untouched object {hash:08x} changed",
                context()
            );
        }
    }

    counts.prop_chunks += 1;
    counts.bytes += data.len();
}

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn every_shipped_prop_rewrites_through_a_delta() {
    let Ok(game_dir) = std::env::var(GAME_DIR) else {
        panic!("set {GAME_DIR} to the client's Game directory");
    };

    let mut wad_files = Vec::new();
    wad_paths(Path::new(&game_dir), &mut wad_files);
    wad_files.sort();
    assert!(!wad_files.is_empty(), "no .wad.client under {game_dir}");

    let mut counts = DeltaCounts::default();
    for wad_path in &wad_files {
        counts.wads += 1;

        let source = File::open(wad_path).expect("the wad opens");
        let mut wad = Wad::mount(source).expect("the wad mounts");
        let chunks: Vec<_> = wad.chunks().as_slice().to_vec();

        for chunk in &chunks {
            let Ok(data) = wad.load_chunk_decompressed(chunk) else {
                continue;
            };
            if BinKind::identify_from_bytes(&data) != Some(BinKind::Prop) {
                continue;
            }
            check_delta_rewrite(wad_path, &data, &mut counts);
        }
    }

    println!("{counts}");
    assert!(counts.prop_chunks > 0, "no PROP chunks in {game_dir}");
}
