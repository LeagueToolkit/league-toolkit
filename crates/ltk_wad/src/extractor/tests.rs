//! Tests for chunk extraction: naming, layout, filtering and progress.

use super::*;
use crate::WadChunks;
use std::io::{Read, Seek, SeekFrom, Write};

// =============================================================================
// Mock WAD Source for Testing
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
// is_hex_chunk_path Tests
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
// PathResolver Tests
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

// =============================================================================
// build_ltk_name Tests
// =============================================================================

#[test]
fn test_build_ltk_name() {
    // Unknown type
    assert_eq!(build_ltk_name("myfile", &[]), "myfile.ltk");

    // PNG magic bytes
    let png_magic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    assert_eq!(build_ltk_name("myfile", &png_magic), "myfile.ltk.png");
}

#[test]
fn test_build_ltk_name_various_types() {
    // JPEG magic
    let jpg_magic = [0xFF, 0xD8, 0xFF, 0xE0];
    assert_eq!(build_ltk_name("image", &jpg_magic), "image.ltk.jpg");

    // DDS magic
    let dds_magic = [0x44, 0x44, 0x53, 0x20]; // "DDS "
    assert_eq!(build_ltk_name("texture", &dds_magic), "texture.ltk.dds");
}

// =============================================================================
// ExtractProgress Tests
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
// WadExtractor Integration Tests
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
            Some(Utf8PathBuf::from("assets/noextension.ltk.png")),
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
fn test_extract_path_without_extension_gets_ltk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = one_chunk_wad(0x1234, "assets/noextension", &PNG_MAGIC);

    WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert!(temp_dir.path().join("assets/noextension.ltk.png").exists());
}

#[test]
fn test_extract_path_without_extension_unknown_type() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) =
        one_chunk_wad(0x1234, "assets/noextension", b"Unknown file type content");

    WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
        .unwrap();

    assert!(temp_dir.path().join("assets/noextension.ltk").exists());
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
// Report, Layout, Policy and Selection Tests
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

#[test]
fn a_failed_write_names_the_chunk() {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
    let (mut wad, resolver) = three_file_wad();

    /* A file where the first chunk's directory has to go. */
    fs::write(temp_dir.path().join("dir1"), "in the way").unwrap();

    let error = WadExtractor::new(&resolver)
        .extract_all(&mut wad, output_path)
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
