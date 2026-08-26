//! Tests for chunk extraction: naming, layout, filtering and progress.

use super::{
    naming::{is_evil, ltk_name, ltk_path, plain_path, DirectoryPaths},
    *,
};
use crate::WadChunks;
use ltk_hash::Hash as _;
use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fs,
    io::{self, Read, Seek, SeekFrom, Write},
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    sync::{Arc, Mutex},
};

// =============================================================================
// A mock WAD source for testing
// =============================================================================

/// A mock WAD source that holds chunk data at specific offsets.
struct MockWadSource {
    data: Vec<u8>,
    position: u64,
}

impl MockWadSource {
    fn new() -> Self {
        Self {
            data: vec![0; 1024 * 1024], // 1MB buffer
            position: 0,
        }
    }

    /// Write data at a specific offset and return the offset.
    fn write_at(&mut self, offset: usize, data: &[u8]) -> usize {
        if offset + data.len() > self.data.len() {
            self.data.resize(offset + data.len(), 0);
        }
        self.data[offset..offset + data.len()].copy_from_slice(data);
        offset
    }

    /// Write gzip-compressed data at a specific offset.
    fn write_gzip_at(&mut self, offset: usize, data: &[u8]) -> (usize, usize) {
        use flate2::write::GzEncoder;
        use flate2::Compression;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        let compressed = encoder.finish().unwrap();
        let compressed_size = compressed.len();
        self.write_at(offset, &compressed);
        (offset, compressed_size)
    }

    /// Create a mock Wad from this source with the given chunks.
    fn into_wad(self, chunks: WadChunks) -> Wad<Self> {
        Wad {
            chunks,
            checksum: 0u64,
            signature: [0u8; 256],
            subchunk_toc: None,
            source: self,
            decoder: ChunkDecoder::new(),
        }
    }
}

impl Read for MockWadSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let start = self.position as usize;
        let end = (start + buf.len()).min(self.data.len());
        let bytes_read = end - start;
        buf[..bytes_read].copy_from_slice(&self.data[start..end]);
        self.position += bytes_read as u64;
        Ok(bytes_read)
    }
}

impl Seek for MockWadSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let new_pos = match pos {
            SeekFrom::Start(p) => p as i64,
            SeekFrom::End(p) => self.data.len() as i64 + p,
            SeekFrom::Current(p) => self.position as i64 + p,
        };
        if new_pos < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "negative seek position",
            ));
        }
        self.position = new_pos as u64;
        Ok(self.position)
    }
}

/// Create a test chunk with uncompressed data.
fn create_uncompressed_chunk(path_hash: u64, data_offset: usize, data: &[u8]) -> WadChunk {
    WadChunk {
        path_hash: WadHash(path_hash),
        data_offset,
        compressed_size: data.len(),
        uncompressed_size: data.len(),
        compression_type: crate::WadChunkCompression::None,
        is_duplicated: false,
        frame_count: 0,
        start_frame: 0,
        checksum: 0,
    }
}

/// Create a test chunk with gzip-compressed data.
fn create_gzip_chunk(
    path_hash: u64,
    data_offset: usize,
    compressed_size: usize,
    uncompressed_size: usize,
) -> WadChunk {
    WadChunk {
        path_hash: WadHash(path_hash),
        data_offset,
        compressed_size,
        uncompressed_size,
        compression_type: crate::WadChunkCompression::GZip,
        is_duplicated: false,
        frame_count: 0,
        start_frame: 0,
        checksum: 0,
    }
}

/// A resolver over the given names.
fn names(entries: &[(u64, &str)]) -> HashMap<WadHash, String> {
    entries
        .iter()
        .map(|(hash, path)| (WadHash(*hash), (*path).to_owned()))
        .collect()
}

/// A wad of one uncompressed chunk at `path_hash`, with its resolver.
fn one_chunk_wad(
    path_hash: u64,
    path: &str,
    data: &[u8],
) -> (Wad<MockWadSource>, HashMap<WadHash, String>) {
    let mut source = MockWadSource::new();
    let offset = source.write_at(1000, data);
    let chunks = WadChunks::from_iter([create_uncompressed_chunk(path_hash, offset, data)]);
    (source.into_wad(chunks), names(&[(path_hash, path)]))
}

// =============================================================================
// Recognising a hex chunk path
// =============================================================================

#[test]
fn test_is_hex_path() {
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef")));
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789ABCDEF")));
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef.bin")));

    assert!(!is_hex_chunk_path(Utf8Path::new("0123456789abcde"))); // too short
    assert!(!is_hex_chunk_path(Utf8Path::new("0123456789abcdefg"))); // too long
    assert!(!is_hex_chunk_path(Utf8Path::new(
        "assets/champions/aatrox.bin"
    )));
    assert!(!is_hex_chunk_path(Utf8Path::new("")));
}

#[test]
fn test_is_hex_path_with_extension() {
    // Hex paths with extensions should still be detected
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef.bin")));
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef.png")));
    assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef.unknown")));
}

#[test]
fn test_is_hex_path_edge_cases() {
    // All zeros
    assert!(is_hex_chunk_path(Utf8Path::new("0000000000000000")));
    // All f's
    assert!(is_hex_chunk_path(Utf8Path::new("ffffffffffffffff")));
    // Non-hex characters
    assert!(!is_hex_chunk_path(Utf8Path::new("ghijklmnopqrstuv")));
    assert!(!is_hex_chunk_path(Utf8Path::new("0123456789abcdeg")));
}

// =============================================================================
// Resolving paths
// =============================================================================

#[test]
fn no_resolver_names_nothing() {
    assert_eq!(NoResolver.resolve(WadHash(0x0123456789abcdef)), None);
    assert!(!NoResolver.is_known(WadHash(0x0123456789abcdef)));
}

#[test]
fn a_hash_map_is_a_resolver() {
    let resolver = names(&[(0x1234, "assets/test.bin")]);

    assert_eq!(
        resolver.resolve(WadHash(0x1234)).as_deref(),
        Some("assets/test.bin")
    );
    assert_eq!(resolver.resolve(WadHash(0x5678)), None);
    assert!(resolver.is_known(WadHash(0x1234)));
    assert!(!resolver.is_known(WadHash(0x5678)));
}

#[test]
fn references_boxes_and_arcs_of_a_resolver_are_resolvers() {
    let map = names(&[(0x1, "one")]);
    let by_ref = &map;
    let boxed: Box<dyn PathResolver> = Box::new(map.clone());
    let shared: Arc<dyn PathResolver> = Arc::new(map.clone());

    let resolvers: [&dyn PathResolver; 3] = [&by_ref, &boxed, &shared];
    for resolver in resolvers {
        assert_eq!(resolver.resolve(WadHash(0x1)).as_deref(), Some("one"));
        assert!(resolver.is_known(WadHash(0x1)));
        assert!(!resolver.is_known(WadHash(0x2)));
    }
}

/// A resolver that records how it was asked, so a test can tell one batch from
/// a run of single lookups.
#[derive(Debug, Default)]
struct CountingResolver {
    names: HashMap<WadHash, String>,
    singles: AtomicUsize,
    /// The size of each batch, in the order the batches arrived.
    batches: Mutex<Vec<usize>>,
}

impl CountingResolver {
    fn new(entries: &[(u64, &str)]) -> Self {
        Self {
            names: names(entries),
            ..Default::default()
        }
    }

    fn batches(&self) -> Vec<usize> {
        self.batches.lock().unwrap().clone()
    }
}

impl PathResolver for CountingResolver {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        self.singles.fetch_add(1, AtomicOrdering::Relaxed);
        self.names.get(&path_hash).cloned()
    }

    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        self.batches.lock().unwrap().push(path_hashes.len());
        path_hashes
            .iter()
            .map(|hash| self.names.get(hash).cloned())
            .collect()
    }
}

/// A resolver that breaks the count the trait promises.
#[derive(Debug)]
struct ShortResolver;

impl PathResolver for ShortResolver {
    fn resolve(&self, _path_hash: WadHash) -> Option<String> {
        None
    }

    fn resolve_all(&self, _path_hashes: &[WadHash]) -> Vec<Option<String>> {
        Vec::new()
    }
}

#[test]
fn the_default_batch_answers_each_hash_through_resolve() {
    let resolver = names(&[(0x1, "one"), (0x3, "three")]);
    let asked = [WadHash(0x1), WadHash(0x2), WadHash(0x3)];

    let resolved = resolver.resolve_all(&asked);

    assert_eq!(
        resolved,
        [Some("one".to_owned()), None, Some("three".to_owned())]
    );
}

#[test]
fn no_resolver_answers_a_batch_with_a_miss_per_hash() {
    let resolved = NoResolver.resolve_all(&[WadHash(0x1), WadHash(0x2)]);

    assert_eq!(resolved, [None, None]);
}

#[test]
fn an_extraction_asks_for_every_chunk_in_one_batch() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, _) = three_file_wad();
    let resolver = CountingResolver::new(&[
        (0x1111, "dir1/file1.txt"),
        (0x2222, "dir2/file2.txt"),
        (0x3333, "dir3/file3.txt"),
    ]);

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 3);
    assert_eq!(resolver.batches(), [3]);
    assert_eq!(resolver.singles.load(AtomicOrdering::Relaxed), 0);
    assert!(temp_dir.path().join("dir1/file1.txt").exists());
    assert!(temp_dir.path().join("dir3/file3.txt").exists());
}

/// The recovery reads every chunk's name before it opens a bin, so it is the
/// second batch and not a lookup per chunk.
#[test]
fn name_recovery_asks_for_every_chunk_in_one_batch() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, _) = three_file_wad();
    let resolver = CountingResolver::new(&[
        (0x1111, "dir1/file1.txt"),
        (0x2222, "dir2/file2.txt"),
        (0x3333, "dir3/file3.txt"),
    ]);

    WadExtractor::new(&resolver)
        .with_name_recovery()
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(resolver.batches(), [3, 3]);
    assert_eq!(resolver.singles.load(AtomicOrdering::Relaxed), 0);
}

/// Without this the extractor, which holds a `&dyn`, would silently take the
/// per-hash default from a resolver that overrode the batch.
#[test]
fn references_boxes_and_arcs_forward_the_batch() {
    let asked = [WadHash(0x1)];
    let by_ref = &CountingResolver::new(&[(0x1, "one")]);
    let boxed: Box<dyn PathResolver> = Box::new(CountingResolver::new(&[(0x1, "one")]));
    let shared: Arc<dyn PathResolver> = Arc::new(CountingResolver::new(&[(0x1, "one")]));

    let resolvers: [&dyn PathResolver; 3] = [&by_ref, &boxed, &shared];
    for resolver in resolvers {
        assert_eq!(resolver.resolve_all(&asked), [Some("one".to_owned())]);
    }

    /* The counter the reference points at is the only one a test can read
    back, and one batch reached it rather than a lookup. */
    assert_eq!(by_ref.batches(), [1]);
    assert_eq!(by_ref.singles.load(AtomicOrdering::Relaxed), 0);
}

#[test]
#[should_panic(expected = "resolve_all answered 0 of 3 hashes")]
fn a_resolver_that_answers_short_stops_the_extraction() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, _) = three_file_wad();

    let _ = WadExtractor::new(&ShortResolver).extract_all(&mut wad, output_path);
}

// =============================================================================
// The .ltk suffix
// =============================================================================

/// The suffix is added and never substituted, so what it is added to always
/// survives. That is what lets a caller hash an extracted file's path back to
/// the chunk it came from.
#[test]
fn the_ltk_suffix_keeps_the_whole_name() {
    assert_eq!(ltk_name("myfile"), "myfile.ltk");
    assert_eq!(ltk_name("myfile.bin"), "myfile.bin.ltk");
    assert_eq!(ltk_name("myfile.tex.dds"), "myfile.tex.dds.ltk");
}

/// Stripping a trailing `.ltk` gives back exactly the name it was built from,
/// whatever that name held.
#[test]
fn the_ltk_suffix_strips_back_to_the_original_name() {
    for original in ["myfile", "myfile.bin", "texture.dds", "a.b.c"] {
        let renamed = ltk_name(original);
        assert_eq!(renamed.strip_suffix(".ltk"), Some(original), "{renamed}");
    }
}

// =============================================================================
// Progress reporting
// =============================================================================

fn progress(done: usize, total: usize) -> ExtractProgress<'static> {
    ExtractProgress {
        done,
        total,
        path_hash: WadHash(0x1234),
        path: "test/path.bin",
        named: true,
        output_path: Some(Utf8Path::new("test/path.bin")),
        result: ExtractResult::Extracted,
        bytes: 42,
    }
}

#[test]
fn fraction_is_done_over_total() {
    assert!((progress(50, 100).fraction() - 0.5).abs() < f64::EPSILON);
    assert!((progress(0, 100).fraction() - 0.0).abs() < f64::EPSILON);
    assert!((progress(100, 100).fraction() - 1.0).abs() < f64::EPSILON);
    assert!((progress(0, 0).fraction() - 0.0).abs() < f64::EPSILON);
}

#[test]
fn progress_accessors_read_the_fields() {
    let progress = progress(1, 2);

    assert_eq!(progress.done(), 1);
    assert_eq!(progress.total(), 2);
    assert_eq!(progress.path_hash(), WadHash(0x1234));
    assert_eq!(progress.path(), "test/path.bin");
    assert!(progress.is_named());
    assert_eq!(progress.output_path(), Some(Utf8Path::new("test/path.bin")));
    assert_eq!(progress.result(), ExtractResult::Extracted);
    assert_eq!(progress.bytes(), 42);
}

// =============================================================================
// Extracting end to end
// =============================================================================

#[test]
fn test_extract_uncompressed_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234567890abcdef, "test/hello.txt", b"Hello, World!");

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    let extracted_path = temp_dir.path().join("test/hello.txt");
    assert_eq!(
        fs::read_to_string(&extracted_path).unwrap(),
        "Hello, World!"
    );
}

#[test]
fn test_extract_gzip_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let test_data = b"This is gzip compressed data!";
    let mut source = MockWadSource::new();
    let (offset, compressed_size) = source.write_gzip_at(1000, test_data);
    let chunk = create_gzip_chunk(0xabcdef1234567890, offset, compressed_size, test_data.len());
    let mut wad = source.into_wad(WadChunks::from_iter([chunk]));
    let resolver = names(&[(0xabcdef1234567890, "compressed/data.txt")]);

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    let extracted_path = temp_dir.path().join("compressed/data.txt");
    assert_eq!(
        fs::read_to_string(&extracted_path).unwrap(),
        "This is gzip compressed data!"
    );
}

#[test]
fn test_extract_all_chunks() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 3);
    assert!(temp_dir.path().join("dir1/file1.txt").exists());
    assert!(temp_dir.path().join("dir2/file2.txt").exists());
    assert!(temp_dir.path().join("dir3/file3.txt").exists());
}

#[test]
fn test_extract_with_path_filter() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset1 = source.write_at(1000, b"Assets file");
    let offset2 = source.write_at(2000, b"Data file");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, b"Assets file"),
        create_uncompressed_chunk(0x2222, offset2, b"Data file"),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "assets/file1.txt"), (0x2222, "data/file2.txt")]);

    let report = WadExtractor::new(&resolver)
        .with_filter(|path| path.starts_with("assets/"))
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert_eq!(report.skipped_by_filter, 1);
    assert!(temp_dir.path().join("assets/file1.txt").exists());
    assert!(!temp_dir.path().join("data/file2.txt").exists());
}

#[test]
fn test_extract_with_type_filter() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let other_data = b"Random text data";
    let offset1 = source.write_at(1000, &PNG_MAGIC);
    let offset2 = source.write_at(2000, other_data);
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, &PNG_MAGIC),
        create_uncompressed_chunk(0x2222, offset2, other_data),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "images/test.png"), (0x2222, "text/readme.txt")]);

    let report = WadExtractor::new(&resolver)
        .with_type_filter([LeagueFileKind::Png])
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert_eq!(report.skipped_by_filter, 1);
    assert!(temp_dir.path().join("images/test.png").exists());
    assert!(!temp_dir.path().join("text/readme.txt").exists());
}

#[test]
fn test_extract_progress_callback() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "test.txt", b"Test data");

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push((
            progress.done(),
            progress.total(),
            progress.path().to_owned(),
            progress.result(),
            progress.bytes(),
        ));
    });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen,
        vec![(1, 1, "test.txt".to_owned(), ExtractResult::Extracted, 9)]
    );
}

#[test]
fn test_extract_hex_path_gets_extension() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset = source.write_at(1000, &PNG_MAGIC);
    let chunk = create_uncompressed_chunk(0x1234567890abcdef, offset, &PNG_MAGIC);
    let mut wad = source.into_wad(WadChunks::from_iter([chunk]));

    let report = WadExtractor::new(&NoResolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert!(temp_dir.path().join("1234567890abcdef.png").exists());
}

#[test]
fn a_named_path_with_a_hex_stem_keeps_its_name() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/0123456789abcdef.txt", &PNG_MAGIC);

    WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert!(temp_dir.path().join("assets/0123456789abcdef.txt").exists());
    assert!(!temp_dir.path().join("assets/0123456789abcdef.png").exists());
}

#[test]
fn progress_reports_a_resolved_name_and_where_it_landed() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/hello.txt", b"Hello");

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push((
            progress.path().to_owned(),
            progress.is_named(),
            progress.output_path().map(Utf8Path::to_path_buf),
        ));
    });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen,
        vec![(
            "assets/hello.txt".to_owned(),
            true,
            Some(Utf8PathBuf::from("assets/hello.txt")),
        )]
    );
}

#[test]
fn progress_reports_an_unresolved_hash_and_where_it_landed() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset = source.write_at(1000, &PNG_MAGIC);
    let chunk = create_uncompressed_chunk(0x1234567890abcdef, offset, &PNG_MAGIC);
    let mut wad = source.into_wad(WadChunks::from_iter([chunk]));

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&NoResolver).on_progress(|progress| {
        seen.push((
            progress.path().to_owned(),
            progress.is_named(),
            progress.output_path().map(Utf8Path::to_path_buf),
        ));
    });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen,
        vec![(
            "1234567890abcdef".to_owned(),
            false,
            Some(Utf8PathBuf::from("1234567890abcdef.png")),
        )]
    );
}

/// The one a name heuristic over the output tree gets wrong: a real name of
/// sixteen hex digits reads as a hash, and its extension is overwritten.
#[test]
fn progress_calls_a_hex_stem_from_the_table_named() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/0123456789abcdef.txt", &PNG_MAGIC);

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push((
            progress.is_named(),
            progress.output_path().map(Utf8Path::to_path_buf),
        ));
    });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen,
        vec![(true, Some(Utf8PathBuf::from("assets/0123456789abcdef.txt")),)]
    );
    assert!(is_hex_chunk_path(Utf8Path::new(
        "assets/0123456789abcdef.txt"
    )));
}

#[test]
fn progress_gives_the_ltk_name_a_chunk_landed_under() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/noextension", &PNG_MAGIC);

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push((
            progress.path().to_owned(),
            progress.output_path().map(Utf8Path::to_path_buf),
        ));
    });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen,
        vec![(
            "assets/noextension".to_owned(),
            Some(Utf8PathBuf::from("assets/noextension")),
        )]
    );
}

#[test]
fn progress_gives_no_output_path_for_a_filtered_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset1 = source.write_at(1000, b"Text data");
    let offset2 = source.write_at(2000, &PNG_MAGIC);
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, b"Text data"),
        create_uncompressed_chunk(0x2222, offset2, &PNG_MAGIC),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "data/notes.txt"), (0x2222, "data/image.png")]);

    let mut seen = BTreeMap::new();
    let mut extractor = WadExtractor::new(&resolver)
        .with_filter(|path| path != "data/notes.txt")
        .with_type_filter([LeagueFileKind::Texture])
        .on_progress(|progress| {
            seen.insert(
                progress.path().to_owned(),
                (
                    progress.result(),
                    progress.is_named(),
                    progress.output_path().map(Utf8Path::to_path_buf),
                ),
            );
        });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen["data/notes.txt"],
        (ExtractResult::SkippedByPath, true, None)
    );
    assert_eq!(
        seen["data/image.png"],
        (ExtractResult::SkippedByType, true, None)
    );
}

#[test]
fn test_extract_creates_nested_directories() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "a/b/c/d/e/deep.txt", b"Deeply nested file");

    WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert!(temp_dir.path().join("a/b/c/d/e/deep.txt").exists());
}

#[test]
fn test_extract_empty_chunks_returns_zero() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let mut wad = MockWadSource::new().into_wad(WadChunks::from_iter([]));

    let report = WadExtractor::new(&NoResolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report, ExtractReport::default());
}

#[test]
fn the_builder_takes_a_filter_on_a_condition() {
    let resolver = NoResolver;
    let mut extractor = WadExtractor::new(&resolver)
        .with_type_filter([LeagueFileKind::Png, LeagueFileKind::Jpeg])
        .on_progress(|_| {});

    let only_assets = true;
    if only_assets {
        extractor = extractor.with_filter(|path| path.starts_with("assets/"));
    }

    let debug = format!("{extractor:?}");
    assert!(debug.contains("WadExtractor"));
    assert!(debug.contains("has_filter: true"));
}

// =============================================================================
// Reports, layouts, policies and selection
// =============================================================================

const PNG_MAGIC: [u8; 12] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

/// Three uncompressed text chunks, each under a directory of its own.
fn three_file_wad() -> (Wad<MockWadSource>, HashMap<WadHash, String>) {
    let mut source = MockWadSource::new();
    let offset1 = source.write_at(1000, b"File one content");
    let offset2 = source.write_at(2000, b"File two content");
    let offset3 = source.write_at(3000, b"File three content");

    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, b"File one content"),
        create_uncompressed_chunk(0x2222, offset2, b"File two content"),
        create_uncompressed_chunk(0x3333, offset3, b"File three content"),
    ]);

    let resolver = names(&[
        (0x1111, "dir1/file1.txt"),
        (0x2222, "dir2/file2.txt"),
        (0x3333, "dir3/file3.txt"),
    ]);

    (source.into_wad(chunks), resolver)
}

#[test]
fn extract_all_reports_counts_bytes_and_kinds() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let text = b"Random text data";
    let offset1 = source.write_at(1000, &PNG_MAGIC);
    let offset2 = source.write_at(2000, text);
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, &PNG_MAGIC),
        create_uncompressed_chunk(0x2222, offset2, text),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "images/test.png"), (0x2222, "text/readme.txt")]);

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert_eq!(report.skipped_existing, 0);
    assert_eq!(report.skipped_by_filter, 0);
    assert!(report.missing.is_empty());
    assert_eq!(report.bytes_written, (PNG_MAGIC.len() + text.len()) as u64);
    assert_eq!(report.by_kind.get(&LeagueFileKind::Png), Some(&1));
    assert_eq!(report.by_kind.values().sum::<usize>(), 2);
    assert!(!report.cancelled);
    assert!(report.recovered.is_empty());
}

#[test]
fn the_report_displays_its_counts() {
    assert_eq!(ExtractReport::default().to_string(), "0 extracted, 0 bytes");

    let report = ExtractReport {
        extracted: 2,
        bytes_written: 40,
        skipped_existing: 1,
        skipped_by_filter: 3,
        missing: vec![WadHash(0x9999)],
        cancelled: true,
        ..Default::default()
    };

    assert_eq!(
        report.to_string(),
        "2 extracted, 40 bytes, 1 existed, 3 filtered out, 1 missing, cancelled"
    );
}

#[test]
fn extract_chunks_takes_a_subset() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let report = WadExtractor::new(&resolver)
        .extract_chunks(&mut wad, [WadHash(0x1111), WadHash(0x3333)], output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert!(temp_dir.path().join("dir1/file1.txt").exists());
    assert!(!temp_dir.path().join("dir2/file2.txt").exists());
    assert!(temp_dir.path().join("dir3/file3.txt").exists());
}

#[test]
fn extract_chunks_lists_the_hashes_the_archive_lacks() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let mut totals = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        totals.push(progress.total());
    });
    let wanted = [WadHash(0x1111), WadHash(0x9999), WadHash(0x1111)];
    let report = extractor
        .extract_chunks(&mut wad, wanted, output_path)
        .unwrap();
    drop(extractor);

    assert_eq!(report.extracted, 1);
    assert_eq!(report.missing, vec![WadHash(0x9999)]);
    assert_eq!(totals, vec![1]);
}

#[test]
fn flat_layout_drops_the_directories() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let report = WadExtractor::new(&resolver)
        .with_layout(ExtractLayout::Flat)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 3);
    assert!(temp_dir.path().join("file1.txt").exists());
    assert!(temp_dir.path().join("file2.txt").exists());
    assert!(temp_dir.path().join("file3.txt").exists());
    assert!(!temp_dir.path().join("dir1").exists());
}

#[test]
fn flat_layout_keeps_two_chunks_of_one_name_apart() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset1 = source.write_at(1000, b"first");
    let offset2 = source.write_at(2000, b"second");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, offset1, b"first"),
        create_uncompressed_chunk(0x2222, offset2, b"second"),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "a/same.txt"), (0x2222, "b/same.txt")]);

    // One worker, so the chunks land in the order the reader reads them.
    let report = WadExtractor::new(&resolver)
        .with_layout(ExtractLayout::Flat)
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("same.txt")).unwrap(),
        "first"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("same.0000000000002222.txt")).unwrap(),
        "second"
    );
}

#[test]
fn a_second_extraction_starts_with_no_flat_names() {
    let first_dir = tempfile::TempDir::new().unwrap();
    let second_dir = tempfile::TempDir::new().unwrap();
    let (mut wad, resolver) = three_file_wad();

    let mut extractor = WadExtractor::new(&resolver).with_layout(ExtractLayout::Flat);
    extractor
        .extract_all(&mut wad, Utf8Path::from_path(first_dir.path()).unwrap())
        .unwrap();
    let report = extractor
        .extract_all(&mut wad, Utf8Path::from_path(second_dir.path()).unwrap())
        .unwrap();

    assert_eq!(report.extracted, 3);
    assert!(second_dir.path().join("file1.txt").exists());
    assert_eq!(fs::read_dir(second_dir.path()).unwrap().count(), 3);
}

#[test]
fn skip_policy_leaves_an_existing_file() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let existing = temp_dir.path().join("dir2/file2.txt");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "kept").unwrap();

    let report = WadExtractor::new(&resolver)
        .with_existing_file_policy(ExistingFilePolicy::Skip)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert_eq!(report.skipped_existing, 1);
    assert_eq!(fs::read_to_string(&existing).unwrap(), "kept");
}

#[test]
fn skip_policy_never_reads_a_chunk_whose_file_exists() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let (offset, compressed_size) = source.write_gzip_at(1000, b"payload");
    let chunk = create_gzip_chunk(0x1111, offset, compressed_size, b"payload".len());
    let resolver = names(&[(0x1111, "dir/file.txt")]);

    let mut wad = source.into_wad(WadChunks::from_iter([chunk]));
    WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    /* The same archive with its bytes ruined. A skip that still read the
    chunk could not decompress it, so a clean report proves the chunk was
    settled by its name alone. */
    let mut ruined = MockWadSource::new();
    ruined.write_at(offset, &vec![0xFF; compressed_size]);
    let mut wad = ruined.into_wad(WadChunks::from_iter([chunk]));
    let report = WadExtractor::new(&resolver)
        .with_existing_file_policy(ExistingFilePolicy::Skip)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.skipped_existing, 1);
    assert_eq!(report.extracted, 0);
}

#[test]
fn overwrite_policy_replaces_an_existing_file() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let existing = temp_dir.path().join("dir2/file2.txt");
    fs::create_dir_all(existing.parent().unwrap()).unwrap();
    fs::write(&existing, "old").unwrap();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 3);
    assert_eq!(report.skipped_existing, 0);
    assert_eq!(fs::read_to_string(&existing).unwrap(), "File two content");
}

#[test]
fn a_set_cancel_flag_stops_before_the_first_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let flag = AtomicBool::new(true);
    let report = WadExtractor::new(&resolver)
        .with_cancel_flag(&flag)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert!(report.cancelled);
    assert_eq!(report.extracted, 0);
    assert!(!temp_dir.path().join("dir1/file1.txt").exists());
}

#[test]
fn progress_reports_each_chunk_once_it_is_done() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver)
        .with_filter(|path| !path.starts_with("dir2/"))
        .on_progress(|progress| {
            seen.push((
                progress.done(),
                progress.total(),
                progress.path().to_owned(),
                progress.result(),
            ));
        });
    extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        seen.iter().map(|(done, ..)| *done).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert!(seen.iter().all(|(_, total, ..)| *total == 3));

    let mut by_path: Vec<_> = seen
        .iter()
        .map(|(_, _, path, result)| (path.as_str(), *result))
        .collect();
    by_path.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(
        by_path,
        vec![
            ("dir1/file1.txt", ExtractResult::Extracted),
            ("dir2/file2.txt", ExtractResult::SkippedByPath),
            ("dir3/file3.txt", ExtractResult::Extracted),
        ]
    );
}

/// A file already standing where a chunk's directory has to go is the same
/// clash as one the extraction makes itself, and it moves the chunk the same
/// way rather than ending the run.
#[test]
fn a_file_blocking_a_directory_displaces_the_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    fs::write(temp_dir.path().join("dir1"), "in the way").unwrap();

    let report = WadExtractor::new(&resolver)
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 3, "{report}");
    assert_eq!(report.displaced.len(), 1, "{report}");
    assert_eq!(report.displaced[0].path, "dir1/file1.txt");
    let PathIssue::Renamed(renamed) = &report.displaced[0].issue else {
        panic!("expected a rename, got {:?}", report.displaced[0].issue);
    };
    assert!(temp_dir.path().join(renamed.as_std_path()).is_file());
    /* The file that was in the way is left as it was. */
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("dir1")).unwrap(),
        "in the way"
    );
}

/// A chunk with nowhere left to go does end the run, and the error says which
/// chunk and which path could not be written.
#[test]
fn a_failed_write_names_the_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    /* The output directory is a file, so even the fallback name has no
    directory to land in. */
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap().join("a-file");
    fs::write(output_path.as_std_path(), "not a directory").unwrap();

    let (mut wad, resolver) = three_file_wad();

    let error = WadExtractor::new(&resolver)
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, &output_path)
        .unwrap_err();

    assert!(
        matches!(
            &error,
            WadError::Chunk { path_hash: WadHash(0x1111), path, .. } if path == "dir1/file1.txt"
        ),
        "{error:?}"
    );
    assert!(error.to_string().contains("dir1/file1.txt"), "{error}");
}

// =============================================================================
// Refusing evil paths
// =============================================================================

/// The shapes a League path takes, which nothing here should stand in the way
/// of. The device names are here because Rust reaches the file system through a
/// verbatim path, which resolves none of them, so they are ordinary files.
#[test]
fn an_ordinary_path_is_not_evil() {
    for path in [
        "assets/characters/aatrox/skin0.bin",
        "a",
        "a/./b",
        "a//b",
        "a/b/",
        "assets/..bin/x...y",
        "data/NUL.bin",
        "data/COM1",
    ] {
        assert!(!is_evil(path), "{path:?} should be allowed");
    }
}

#[test]
fn an_evil_path_is_refused() {
    let bs = "\\";
    let cases = [
        // Names no file at all.
        ("", "empty"),
        (".", "the directory itself"),
        ("./.", "nothing but dots"),
        // Ignores the directory it is joined onto.
        ("/etc/passwd", "unix root"),
        ("C:/evil.bat", "drive"),
        ("c:evil.bat", "drive relative"),
        // Reaches the directory above. Measured: this one really does escape.
        ("..", "bare"),
        ("../evil.bat", "leading"),
        ("assets/../../evil.bat", "in the middle"),
        ("assets/..", "trailing"),
        // A drive or an alternate data stream, not a file the directory lists.
        ("data/notes.txt:stream", "data stream"),
        ("data/c:evil.bat", "drive inside a component"),
        /* Windows strips a trailing dot or space before it looks a name up, so
        `notes.txt.` and `notes.txt` are one file. Refusing only one of the two
        would walk the pair past the check for two chunks claiming one path. */
        ("data/notes.txt.", "trailing dot"),
        ("data/notes.txt ", "trailing space"),
        ("data./notes.txt", "trailing dot on a directory"),
        ("...", "bare dots"),
        (".. ", "dots and a space"),
        ("assets/.../evil.bat", "bare dots in the middle"),
    ];

    for (path, why) in cases {
        assert!(is_evil(path), "{path:?} ({why}) should be refused");
    }

    /* Backslashes count wherever the extraction runs: a table written for
    Windows must not escape on Linux, nor the other way round. */
    for path in [
        format!("..{bs}evil.bat"),
        format!("assets{bs}..{bs}..{bs}evil.bat"),
        format!("{bs}evil.bat"),
        format!("{bs}{bs}server{bs}share{bs}evil.bat"),
    ] {
        assert!(is_evil(&path), "{path:?} should be refused");
    }
}

#[test]
fn a_path_leaving_the_output_directory_is_refused() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "../../../evil.bat", b"payload");

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push((progress.result(), progress.output_path().is_some()));
    });
    let report = extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(report.extracted, 0);
    assert_eq!(report.rejected(), 1);
    assert_eq!(seen, vec![(ExtractResult::SkippedRejectedPath, false)]);
    assert_eq!(
        report.displaced,
        vec![DisplacedChunk {
            path_hash: WadHash(0x1234),
            path: "../../../evil.bat".to_owned(),
            issue: PathIssue::Rejected,
        }]
    );
    assert!(!temp_dir.path().parent().unwrap().join("evil.bat").exists());
}

/// A caller's own filter must not be able to hide a hostile table.
#[test]
fn a_filter_does_not_mask_an_unsafe_path() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "../evil.bat", b"payload");

    let report = WadExtractor::new(&resolver)
        .with_filter(|_| false)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.skipped_by_filter, 0);
    assert_eq!(report.rejected(), 1);
    assert_eq!(report.displaced[0].issue, PathIssue::Rejected);
}

/// Any resolver's paths are untrusted, name recovery's included.
#[test]
fn an_absolute_path_from_any_resolver_is_refused() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, _) = one_chunk_wad(0x1234, "unused", b"payload");

    struct Escaping;
    impl PathResolver for Escaping {
        fn resolve(&self, _path_hash: WadHash) -> Option<String> {
            Some("/tmp/evil.bat".to_owned())
        }
    }

    let report = WadExtractor::new(&Escaping)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.rejected(), 1);
    assert_eq!(report.displaced[0].issue, PathIssue::Rejected);
}

// =============================================================================
// Path collisions
// =============================================================================

#[test]
fn a_second_chunk_claiming_one_path_is_not_written_over_the_first() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let first = source.write_at(1000, b"first");
    let second = source.write_at(2000, b"second");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, first, b"first"),
        create_uncompressed_chunk(0x2222, second, b"second"),
    ]);
    let mut wad = source.into_wad(chunks);
    /* A stale table naming two hashes the same path. */
    let resolver = names(&[(0x1111, "data/notes.txt"), (0x2222, "data/notes.txt")]);

    let report = WadExtractor::new(&resolver)
        /* One worker, so the order chunks claim in is the archive's. */
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert_eq!(report.duplicates(), 1);
    assert_eq!(
        report.displaced,
        vec![DisplacedChunk {
            path_hash: WadHash(0x2222),
            path: "data/notes.txt".to_owned(),
            issue: PathIssue::Duplicate,
        }]
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("data/notes.txt")).unwrap(),
        "first"
    );
}

/// The flat layout collides by design, so it keeps disambiguating rather than
/// dropping the second chunk.
#[test]
fn the_flat_layout_still_suffixes_a_shared_name() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let first = source.write_at(1000, b"first");
    let second = source.write_at(2000, b"second");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, first, b"first"),
        create_uncompressed_chunk(0x2222, second, b"second"),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[(0x1111, "one/notes.txt"), (0x2222, "two/notes.txt")]);

    let report = WadExtractor::new(&resolver)
        .with_layout(ExtractLayout::Flat)
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert!(report.displaced.is_empty());
    assert!(temp_dir.path().join("notes.txt").exists());
    assert!(temp_dir.path().join("notes.0000000000002222.txt").exists());
}

// =============================================================================
// Names the file system refuses
// =============================================================================

#[test]
fn a_name_a_directory_holds_is_renamed_and_reported() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/thing.bin", &PNG_MAGIC);

    /* A directory already standing where the chunk's file would go. */
    fs::create_dir_all(temp_dir.path().join("assets/thing.bin")).unwrap();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert_eq!(
        report.displaced,
        vec![DisplacedChunk {
            path_hash: WadHash(0x1234),
            path: "assets/thing.bin".to_owned(),
            issue: PathIssue::Renamed(Utf8PathBuf::from("assets/thing.bin.ltk")),
        }]
    );
    assert!(temp_dir.path().join("assets/thing.bin.ltk").exists());
}

// =============================================================================
// A path with no extension
// =============================================================================

/// Nothing moves a named chunk off its own path but a directory of that name,
/// so a path with no extension keeps it. A `.ltk` suffix would say no more than
/// the bare path already does, and would cost the caller the path it hashes by.
#[test]
fn a_path_with_no_extension_keeps_it() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/noextension", &PNG_MAGIC);

    let mut seen = Vec::new();
    let mut extractor = WadExtractor::new(&resolver).on_progress(|progress| {
        seen.push(progress.output_path().map(Utf8Path::to_path_buf));
    });
    let report = extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(report.extracted, 1);
    assert!(report.displaced.is_empty());
    assert_eq!(seen, vec![Some(Utf8PathBuf::from("assets/noextension"))]);
    assert!(temp_dir.path().join("assets/noextension").exists());
    assert!(!temp_dir.path().join("assets/noextension.ltk").exists());
}

/// A directory of that name does move it, because a file cannot share a name
/// with one. The report says so rather than let it pass unseen.
#[test]
fn a_directory_of_that_name_appends_the_suffix() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/noextension", &PNG_MAGIC);

    fs::create_dir_all(temp_dir.path().join("assets/noextension")).unwrap();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert_eq!(
        report.displaced[0].issue,
        PathIssue::Renamed(Utf8PathBuf::from("assets/noextension.ltk"))
    );
}

/// The whole point of appending: the original extension survives the rename,
/// so the path can still be hashed back to its chunk.
#[test]
fn the_suffix_keeps_an_extension_the_path_already_had() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/thing.bin", &PNG_MAGIC);

    fs::create_dir_all(temp_dir.path().join("assets/thing.bin")).unwrap();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    let PathIssue::Renamed(landed) = &report.displaced[0].issue else {
        panic!("expected a rename, got {:?}", report.displaced[0].issue);
    };
    assert_eq!(landed.as_path(), Utf8Path::new("assets/thing.bin.ltk"));
    /* The name the chunk was given is what is left when the suffix comes off,
    down to the `.bin` a stem-built name would have dropped. */
    assert_eq!(
        landed
            .file_name()
            .and_then(|name| name.strip_suffix(".ltk")),
        Some("thing.bin")
    );
}

// =============================================================================
// A path that is a directory of another path
// =============================================================================

/// A WAD can name both `x` and `x/y`. No file system holds both, so one chunk
/// has to move. The extraction must finish, not die on the second one,
/// and both paths must come through it.
///
/// Chunks are written in path hash order, so which of the pair carries the
/// lower hash decides which one the writes reach first. Neither assignment
/// changes the tree: `x` is the one that moves, every time.
#[test]
fn the_write_order_of_a_clashing_pair_does_not_change_the_tree() {
    for (plain, nested) in [(0x1111u64, 0x2222u64), (0x2222, 0x1111)] {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let mut source = MockWadSource::new();
        let plain_at = source.write_at(1000, b"the file");
        let nested_at = source.write_at(2000, b"the nested file");
        let chunks = WadChunks::from_iter([
            create_uncompressed_chunk(plain, plain_at, b"the file"),
            create_uncompressed_chunk(nested, nested_at, b"the nested file"),
        ]);
        let mut wad = source.into_wad(chunks);
        let resolver = names(&[(plain, "assets/thing"), (nested, "assets/thing/inner.bin")]);

        let report = WadExtractor::new(&resolver)
            .with_workers(NonZeroUsize::new(1).unwrap())
            .extract_all(&mut wad, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2, "{report}");
        assert_eq!(
            tree(temp_dir.path()),
            ["assets/thing.ltk", "assets/thing/inner.bin"],
            "plain chunk at {plain:#x}"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("assets/thing.ltk")).unwrap(),
            "the file"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("assets/thing/inner.bin")).unwrap(),
            "the nested file"
        );

        /* The chunk that moved is the one a directory has to hold, whichever
        of the two the writes reached first. */
        assert_eq!(report.displaced.len(), 1, "{report}");
        assert_eq!(report.displaced[0].path_hash, WadHash(plain));
        assert_eq!(
            report.displaced[0].issue,
            PathIssue::Renamed(Utf8PathBuf::from("assets/thing.ltk"))
        );
    }
}

/// The same clash for a path that carries an extension, which the suffix goes
/// on the end of rather than replaces. A tool that reads `.bin` files by their
/// name will not find this one: the price of a name that hashes back to its
/// chunk, and one no measured hash table pays, since every clash in a real table is
/// between paths with no extension at all.
#[test]
fn an_extension_does_not_save_a_path_from_the_same_clash() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let first = source.write_at(1000, b"the file");
    let second = source.write_at(2000, b"the nested file");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, first, b"the file"),
        create_uncompressed_chunk(0x2222, second, b"the nested file"),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[
        (0x1111, "assets/thing.bin"),
        (0x2222, "assets/thing.bin/inner.bin"),
    ]);

    let report = WadExtractor::new(&resolver)
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2, "{report}");
    assert_eq!(
        tree(temp_dir.path()),
        ["assets/thing.bin.ltk", "assets/thing.bin/inner.bin"]
    );
    assert_eq!(report.displaced.len(), 1);
    assert_eq!(report.displaced[0].path_hash, WadHash(0x1111));
    assert_eq!(
        report.displaced[0].issue,
        PathIssue::Renamed(Utf8PathBuf::from("assets/thing.bin.ltk"))
    );
}

/// A resolver's paths are untrusted, and a table naming `<hash>/y` for the very
/// hash a nameless chunk lands under clashes with that chunk's own hex name.
/// The nameless chunk moves like any other, rather than ending the run: it has
/// nowhere else to go, since the name a refused write falls back to is the hex
/// name it just failed to write.
#[test]
fn a_directory_named_for_a_nameless_chunk_does_not_end_the_run() {
    /* Either side of the nameless chunk's own hash, so the nested chunk is
    written once before it and once after. */
    for nested_hash in [0x0001u64, 0x2222] {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let mut source = MockWadSource::new();
        /* Bytes of no known kind, so the chunk takes its bare hex name. */
        let nameless = source.write_at(1000, b"the file");
        let nested = source.write_at(2000, b"the nested file");
        let chunks = WadChunks::from_iter([
            create_uncompressed_chunk(0x1111, nameless, b"the file"),
            create_uncompressed_chunk(nested_hash, nested, b"the nested file"),
        ]);
        let mut wad = source.into_wad(chunks);
        let resolver = names(&[(nested_hash, "0000000000001111/inner.bin")]);

        let report = WadExtractor::new(&resolver)
            .with_workers(NonZeroUsize::new(1).unwrap())
            .extract_all(&mut wad, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2, "{report}");
        assert_eq!(
            tree(temp_dir.path()),
            ["0000000000001111.ltk", "0000000000001111/inner.bin"],
            "nested chunk at {nested_hash:#x}"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("0000000000001111.ltk")).unwrap(),
            "the file"
        );

        let displaced = &report.displaced[0];
        assert_eq!(displaced.path_hash, WadHash(0x1111));
        assert_eq!(
            displaced.issue,
            PathIssue::Renamed(Utf8PathBuf::from("0000000000001111.ltk"))
        );
        /* The suffix goes on the end here too, so the name still reads as the
        hash it was built from. */
        assert!(is_hex_chunk_path(Utf8Path::new("0000000000001111.ltk")));
    }
}

/// A path the filter drops is never written, so it makes no directory and the
/// path it would have clashed with keeps its own name.
#[test]
fn a_filter_that_drops_the_nested_path_leaves_the_plain_one_alone() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = clashing_pair_wad();

    let report = WadExtractor::new(&resolver)
        .with_filter(|path| !path.ends_with("inner.bin"))
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1, "{report}");
    assert!(report.displaced.is_empty(), "{report}");
    assert_eq!(tree(temp_dir.path()), ["assets/thing"]);
}

/// A path the extraction refuses makes no directory either.
#[test]
fn a_refused_nested_path_leaves_the_plain_one_alone() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let plain = source.write_at(1000, b"the file");
    let nested = source.write_at(2000, b"the nested file");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, plain, b"the file"),
        create_uncompressed_chunk(0x2222, nested, b"the nested file"),
    ]);
    let mut wad = source.into_wad(chunks);
    let resolver = names(&[
        (0x1111, "assets/thing"),
        (0x2222, "assets/thing/../../inner.bin"),
    ]);

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1, "{report}");
    assert_eq!(report.displaced.len(), 1, "{report}");
    assert_eq!(report.displaced[0].issue, PathIssue::Rejected);
    assert_eq!(tree(temp_dir.path()), ["assets/thing"]);
}

/// Every worker racing on one such pair still finishes, whichever order they
/// happen to reach the file system in.
#[test]
fn racing_workers_on_a_clashing_pair_still_finish() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let mut chunks = Vec::new();
    let mut entries = Vec::new();
    for i in 0..16u64 {
        let payload = format!("payload {i}");
        let offset = source.write_at(1000 + i as usize * 64, payload.as_bytes());
        chunks.push(create_uncompressed_chunk(i, offset, payload.as_bytes()));
        /* Even hashes take a plain name, odd ones make that name a directory. */
        entries.push(if i % 2 == 0 {
            (i, format!("assets/thing{}", i / 2))
        } else {
            (i, format!("assets/thing{}/inner.bin", i / 2))
        });
    }
    let mut wad = source.into_wad(WadChunks::from_iter(chunks));
    let borrowed: Vec<(u64, &str)> = entries
        .iter()
        .map(|(hash, path)| (*hash, path.as_str()))
        .collect();
    let resolver = names(&borrowed);

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 16, "{report}");
    assert_eq!(report.displaced.len(), 8, "{report}");

    let mut expected: Vec<String> = (0..8)
        .flat_map(|i| {
            [
                format!("assets/thing{i}.ltk"),
                format!("assets/thing{i}/inner.bin"),
            ]
        })
        .collect();
    expected.sort();
    assert_eq!(tree(temp_dir.path()), expected);
}

// =============================================================================
// One tree, every run
// =============================================================================

/// Every file under `root`, relative to it and sorted, with `/` between the
/// components whatever the host writes.
fn tree(root: &std::path::Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, prefix: &str, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name().into_string().unwrap();
            let path = format!("{prefix}{name}");
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &format!("{path}/"), found);
            } else {
                found.push(path);
            }
        }
    }

    let mut found = Vec::new();
    walk(root, "", &mut found);
    found.sort();
    found
}

/// A WAD holding both `assets/thing` and `assets/thing/inner.bin`.
fn clashing_pair_wad() -> (Wad<MockWadSource>, HashMap<WadHash, String>) {
    let mut source = MockWadSource::new();
    let plain = source.write_at(1000, b"the file");
    let nested = source.write_at(2000, b"the nested file");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, plain, b"the file"),
        create_uncompressed_chunk(0x2222, nested, b"the nested file"),
    ]);
    let resolver = names(&[(0x1111, "assets/thing"), (0x2222, "assets/thing/inner.bin")]);
    (source.into_wad(chunks), resolver)
}

/// The clash is settled over the extraction's own paths before anything is
/// written, so which of `x` and `x/y` moves is not a race. Either read order
/// gives one tree, and it is the tree that keeps both paths: the one a
/// directory has to hold takes the suffix, the other stays where it is.
#[test]
fn a_clashing_pair_gives_one_tree_whichever_order_it_is_read_in() {
    let orders = [
        [WadHash(0x1111), WadHash(0x2222)],
        [WadHash(0x2222), WadHash(0x1111)],
    ];

    for order in orders {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let (mut wad, resolver) = clashing_pair_wad();

        let report = WadExtractor::new(&resolver)
            .with_workers(NonZeroUsize::new(1).unwrap())
            .extract_chunks(&mut wad, order, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2, "{report}");
        assert_eq!(
            tree(temp_dir.path()),
            ["assets/thing.ltk", "assets/thing/inner.bin"],
            "read in {order:?}"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("assets/thing.ltk")).unwrap(),
            "the file"
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join("assets/thing/inner.bin")).unwrap(),
            "the nested file"
        );

        assert_eq!(report.displaced.len(), 1, "{report}");
        assert_eq!(report.displaced[0].path_hash, WadHash(0x1111));
        assert_eq!(
            report.displaced[0].issue,
            PathIssue::Renamed(Utf8PathBuf::from("assets/thing.ltk"))
        );
    }
}

// =============================================================================
// The directories a set of paths names
// =============================================================================

/// The directories are the path's own prefixes, and the path itself is not one
/// of them: a leaf is only a directory because some *other* path says so.
#[test]
fn the_directories_of_a_path_are_its_prefixes() {
    let directories = DirectoryPaths::of(["assets/champions/aatrox.bin"]);

    assert!(directories.holds("assets"));
    assert!(directories.holds("assets/champions"));
    assert!(!directories.holds("assets/champions/aatrox.bin"));
}

/// A path with no directory in it names none.
#[test]
fn a_bare_name_names_no_directory() {
    let directories = DirectoryPaths::of(["aatrox.bin"]);

    assert!(!directories.holds("aatrox.bin"));
    assert!(!directories.holds(""));
}

/// A hash table written on Windows and one written anywhere else name the same
/// directories, and either is found by a lookup written the other way. The
/// extraction would otherwise clash on one host and not on the other.
#[test]
fn a_backslash_names_the_same_directory_a_slash_does() {
    let from_backslashes = DirectoryPaths::of([r"assets\champions\aatrox.bin"]);

    assert!(from_backslashes.holds("assets/champions"));
    assert!(from_backslashes.holds(r"assets\champions"));

    let from_slashes = DirectoryPaths::of(["assets/champions/aatrox.bin"]);
    assert!(from_slashes.holds(r"assets\champions"));
}

/// The components a join steps over are the ones this steps over, so a path
/// written the long way round still finds the directory it names.
#[test]
fn the_components_a_join_steps_over_name_no_directory() {
    let directories = DirectoryPaths::of(["./assets//champions/aatrox.bin"]);

    assert!(directories.holds("assets"));
    assert!(directories.holds("assets/champions"));
    assert!(directories.holds("./assets//champions"));
    assert!(!directories.holds("."));
    assert!(!directories.holds(""));
}

/// A directory one path names is one whatever the other paths hold.
#[test]
fn paths_pool_the_directories_they_name() {
    let directories = DirectoryPaths::of(["a/b/one.bin", "a/c/two.bin", "d"]);

    for held in ["a", "a/b", "a/c"] {
        assert!(directories.holds(held), "{held}");
    }
    for free in ["d", "a/b/one.bin", "a/d"] {
        assert!(!directories.holds(free), "{free}");
    }
}

/// Nothing at all names nothing at all, which is what a flat layout hands the
/// writer.
#[test]
fn no_paths_name_no_directories() {
    let directories = DirectoryPaths::default();

    assert!(!directories.holds("assets"));
    assert!(!directories.holds(""));
}

// =============================================================================
// Normalising a path
// =============================================================================

/// A path already written with one `/` between its components is handed back
/// as it is, which is nearly every path a hash table holds.
#[test]
fn a_path_that_needs_no_work_is_borrowed() {
    for path in ["assets/champions/aatrox.bin", "aatrox.bin", "a"] {
        assert!(
            matches!(plain_path(path), Cow::Borrowed(_)),
            "{path} was copied"
        );
    }
}

#[test]
fn a_path_is_written_with_one_slash_between_its_components() {
    for (path, plain) in [
        (
            r"assets\champions\aatrox.bin",
            "assets/champions/aatrox.bin",
        ),
        ("assets//champions", "assets/champions"),
        ("./assets/champions", "assets/champions"),
        ("assets/champions/", "assets/champions"),
        (
            r"assets\champions/aatrox.bin",
            "assets/champions/aatrox.bin",
        ),
        ("./", ""),
    ] {
        assert_eq!(plain_path(path), plain, "{path}");
    }
}

// =============================================================================
// The renamed path is still a path
// =============================================================================

/// The suffix goes on the end of the path string, so the path a caller reads
/// back is written the way every other path of the extraction is.
///
/// Building it through `set_file_name` would re-join the path with the host's
/// separator, handing back `assets\thing.bin.ltk` on Windows where every
/// un-renamed chunk reports `assets/thing.bin`. A caller stripping the suffix
/// would then hash `assets\thing.bin`, which is not the chunk's path hash.
#[test]
fn a_renamed_path_keeps_the_separators_it_came_with() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = clashing_pair_wad();

    let report = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    let PathIssue::Renamed(landed) = &report.displaced[0].issue else {
        panic!("expected a rename, got {:?}", report.displaced[0].issue);
    };
    assert_eq!(landed.as_str(), "assets/thing.ltk");
    assert_eq!(landed.as_str().strip_suffix(".ltk"), Some("assets/thing"));
}

/// A table can name the suffixed path a directory too. There is nothing left
/// to suffix onto (a second `.ltk` would no longer strip back to the path),
/// so the chunk takes its hash, the name any refused write falls to. It must
/// not fall there by failing a write, which is the order-dependent branch the
/// pre-pass exists to remove.
#[test]
fn a_directory_over_the_suffixed_name_sends_the_chunk_to_its_hash() {
    for order in [[0x1111u64, 0x2222, 0x3333], [0x3333, 0x2222, 0x1111]] {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let mut source = MockWadSource::new();
        let plain = source.write_at(1000, b"the file");
        let nested = source.write_at(2000, b"the nested file");
        let over = source.write_at(3000, b"over the suffix");
        let chunks = WadChunks::from_iter([
            create_uncompressed_chunk(order[0], plain, b"the file"),
            create_uncompressed_chunk(order[1], nested, b"the nested file"),
            create_uncompressed_chunk(order[2], over, b"over the suffix"),
        ]);
        let mut wad = source.into_wad(chunks);
        let resolver = names(&[
            (order[0], "assets/thing"),
            (order[1], "assets/thing/inner.bin"),
            (order[2], "assets/thing.ltk/also.bin"),
        ]);

        let report = WadExtractor::new(&resolver)
            .extract_all(&mut wad, output_path)
            .unwrap();

        assert_eq!(report.extracted, 3, "{report}");
        let mut expected = [
            format!("{:016x}", order[0]),
            "assets/thing/inner.bin".to_owned(),
            "assets/thing.ltk/also.bin".to_owned(),
        ];
        expected.sort();
        assert_eq!(
            tree(temp_dir.path()),
            expected,
            "plain chunk {:#x}",
            order[0]
        );

        let displaced = &report.displaced[0];
        assert_eq!(displaced.path_hash, WadHash(order[0]));
        assert_eq!(
            displaced.issue,
            PathIssue::Renamed(Utf8PathBuf::from(format!("{:016x}", order[0])))
        );
        assert_eq!(
            fs::read_to_string(temp_dir.path().join(format!("{:016x}", order[0]))).unwrap(),
            "the file"
        );
    }
}

// =============================================================================
// Asking the caller's code once
// =============================================================================

/// Resolving is the work an extraction asks a caller's code to do most often,
/// and a resolver can be doing far more per lookup than a hash map does. It is
/// asked once per chunk, and so is the path filter, whatever the paths turn
/// out to clash over.
///
/// The rename cannot be settled chunk by chunk, since whether a path has to
/// move depends on every other path of the extraction. That is why the names
/// are read up front, but up front is not twice.
#[test]
fn each_chunk_is_resolved_and_filtered_once() {
    struct CountingResolver {
        names: HashMap<WadHash, String>,
        calls: std::cell::Cell<usize>,
    }

    impl PathResolver for CountingResolver {
        fn resolve(&self, path_hash: WadHash) -> Option<String> {
            self.calls.set(self.calls.get() + 1);
            self.names.get(&path_hash).cloned()
        }
    }

    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let plain = source.write_at(1000, b"the file");
    let nested = source.write_at(2000, b"the nested file");
    let other = source.write_at(3000, b"another file");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, plain, b"the file"),
        create_uncompressed_chunk(0x2222, nested, b"the nested file"),
        create_uncompressed_chunk(0x3333, other, b"another file"),
    ]);
    let mut wad = source.into_wad(chunks);

    let resolver = CountingResolver {
        names: names(&[
            (0x1111, "assets/thing"),
            (0x2222, "assets/thing/inner.bin"),
            (0x3333, "assets/other.bin"),
        ]),
        calls: std::cell::Cell::new(0),
    };
    let filter_calls = std::cell::Cell::new(0);

    let mut extractor = WadExtractor::new(&resolver).with_filter(|path| {
        filter_calls.set(filter_calls.get() + 1);
        path != "assets/other.bin"
    });
    let report = extractor.extract_all(&mut wad, output_path).unwrap();
    drop(extractor);

    assert_eq!(
        resolver.calls.get(),
        3,
        "resolver asked more than once a chunk"
    );
    assert_eq!(filter_calls.get(), 3, "filter asked more than once a chunk");

    /* And the clash is still settled, which is what the up-front pass is for. */
    assert_eq!(report.extracted, 2, "{report}");
    assert_eq!(
        tree(temp_dir.path()),
        ["assets/thing.ltk", "assets/thing/inner.bin"]
    );
}

#[test]
fn lossless_naming_keeps_a_chunk_whose_path_is_taken() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let first = source.write_at(1000, b"first");
    let second = source.write_at(2000, b"second");
    let chunks = WadChunks::from_iter([
        create_uncompressed_chunk(0x1111, first, b"first"),
        create_uncompressed_chunk(0x2222, second, b"second"),
    ]);
    let mut wad = source.into_wad(chunks);
    /* A stale table naming two hashes the same path. */
    let resolver = names(&[(0x1111, "data/notes.txt"), (0x2222, "data/notes.txt")]);

    let report = WadExtractor::new(&resolver)
        .with_naming_policy(NamingPolicy::Lossless)
        /* One worker, so the order chunks claim in is the archive's. */
        .with_workers(NonZeroUsize::new(1).unwrap())
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 2);
    assert_eq!(report.duplicates(), 0);
    assert_eq!(report.renamed(), 1);
    assert_eq!(
        report.displaced,
        vec![DisplacedChunk {
            path_hash: WadHash(0x2222),
            path: "data/notes.txt".to_owned(),
            issue: PathIssue::Renamed(Utf8PathBuf::from("data/notes.txt.ltk")),
        }]
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("data/notes.txt")).unwrap(),
        "first"
    );
    assert_eq!(
        fs::read_to_string(temp_dir.path().join("data/notes.txt.ltk")).unwrap(),
        "second"
    );
}

#[test]
fn lossless_naming_leaves_a_nameless_chunk_without_an_extension() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

    let mut source = MockWadSource::new();
    let offset = source.write_at(1000, &PNG_MAGIC);
    let chunk = create_uncompressed_chunk(0x1234567890abcdef, offset, &PNG_MAGIC);
    let mut wad = source.into_wad(WadChunks::from_iter([chunk]));

    let report = WadExtractor::new(&NoResolver)
        .with_naming_policy(NamingPolicy::Lossless)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    assert!(temp_dir.path().join("1234567890abcdef").exists());
    assert!(!temp_dir.path().join("1234567890abcdef.png").exists());
}

#[test]
fn hex_chunk_hash_reads_back_what_hex_name_wrote() {
    let hash = WadHash(0xff);
    let name = hex_name(hash);

    assert_eq!(name, "00000000000000ff");
    assert_eq!(hex_chunk_hash(Utf8Path::new(&name)), Some(hash));
    assert_eq!(
        hex_chunk_hash(Utf8Path::new("00000000000000ff.dds")),
        Some(hash)
    );
    assert_eq!(hex_chunk_hash(Utf8Path::new("assets/aatrox.bin")), None);
    /* Fifteen digits is not a chunk name, so a short hex stem is not one. */
    assert_eq!(hex_chunk_hash(Utf8Path::new("00000000000ff")), None);
}

#[test]
fn chunk_hash_of_undoes_every_name_the_extraction_gives() {
    /* A nameless chunk, under either naming policy. */
    let hash = WadHash(0x0123456789abcdef);
    assert_eq!(chunk_hash_of(Utf8Path::new("0123456789abcdef")), hash);
    assert_eq!(chunk_hash_of(Utf8Path::new("0123456789abcdef.png")), hash);
    /* The same chunk once a directory took its name. */
    assert_eq!(chunk_hash_of(Utf8Path::new("0123456789abcdef.ltk")), hash);

    /* A path a resolver gave, and the same path after a rename. */
    let named = WadHash::hash_str("assets/thing.bin");
    assert_eq!(chunk_hash_of(Utf8Path::new("assets/thing.bin")), named);
    assert_eq!(chunk_hash_of(Utf8Path::new("assets/thing.bin.ltk")), named);
}

#[test]
fn strip_ltk_suffix_is_the_inverse_of_the_rename() {
    let path = Utf8Path::new("assets/thing.bin");

    assert_eq!(strip_ltk_suffix(&ltk_path(path)), path);
    /* Nothing to strip leaves the path as it is. */
    assert_eq!(strip_ltk_suffix(path), path);
}

#[test]
fn merging_reports_adds_every_count() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    let one = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();
    let two = WadExtractor::new(&resolver)
        .with_existing_file_policy(ExistingFilePolicy::Skip)
        .extract_all(&mut wad, output_path)
        .unwrap();

    let mut totals = ExtractReport::default();
    totals.merge(one);
    totals.merge(two);

    assert_eq!(totals.extracted, 3);
    assert_eq!(totals.skipped_existing, 3);
    assert_eq!(totals.by_kind.values().sum::<usize>(), 3);
}

// =============================================================================
// ZstdMulti extraction by subchunk records
// =============================================================================

/// A `ZstdMulti` chunk whose raw first subchunk holds the zstd magic, with the
/// table that describes it, and the bytes the chunk holds.
#[cfg(feature = "zstd")]
fn zstd_multi_wad(workers: usize) -> (Wad<MockWadSource>, Vec<u8>, NonZeroUsize) {
    use crate::{SubchunkToc, WadChunkCompression};

    let raw_run = b"raw (\xb5/\xfda fake frame start".to_vec();
    let content = b"subchunked content".repeat(20);
    let frame = zstd::encode_all(io::Cursor::new(&content[..]), 3).unwrap();
    let chunk_data: Vec<u8> = [raw_run.as_slice(), &frame].concat();
    let expected: Vec<u8> = [raw_run.as_slice(), &content].concat();

    let mut toc_data = Vec::new();
    for (compressed, uncompressed) in [
        (raw_run.len() as u32, raw_run.len() as u32),
        (frame.len() as u32, content.len() as u32),
    ] {
        toc_data.extend_from_slice(&compressed.to_le_bytes());
        toc_data.extend_from_slice(&uncompressed.to_le_bytes());
        toc_data.extend_from_slice(&0u64.to_le_bytes());
    }

    let mut source = MockWadSource::new();
    let offset = source.write_at(1000, &chunk_data);
    let chunks = WadChunks::from_iter([WadChunk {
        path_hash: WadHash(0x1111),
        data_offset: offset,
        compressed_size: chunk_data.len(),
        uncompressed_size: expected.len(),
        compression_type: WadChunkCompression::ZstdMulti,
        is_duplicated: false,
        frame_count: 2,
        start_frame: 0,
        checksum: 0,
    }]);

    let mut wad = source.into_wad(chunks);
    wad.subchunk_toc = Some(SubchunkToc::from_bytes(&toc_data).unwrap());
    (wad, expected, NonZeroUsize::new(workers).unwrap())
}

/// The raw run's fake frame start defeats the magic-scan fallback, so this
/// passes only when the workers decode by the subchunk records.
#[test]
#[cfg(feature = "zstd")]
fn a_zstd_multi_chunk_extracts_by_its_subchunk_records() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, expected, workers) = zstd_multi_wad(4);

    let report = WadExtractor::new(&names(&[(0x1111, "assets/data.bin")]))
        .with_workers(workers)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert_eq!(report.extracted, 1);
    let written = fs::read(temp_dir.path().join("assets/data.bin")).unwrap();
    assert_eq!(written, expected);
}

/// Without the table the same chunk fails, so the fallback does not quietly
/// write garbage.
#[test]
#[cfg(feature = "zstd")]
fn a_zstd_multi_chunk_without_the_table_still_goes_to_the_scan() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, _, workers) = zstd_multi_wad(1);
    wad.subchunk_toc = None;

    let result = WadExtractor::new(&names(&[(0x1111, "assets/data.bin")]))
        .with_workers(workers)
        .extract_all(&mut wad, output_path);
    result.unwrap_err();
}
