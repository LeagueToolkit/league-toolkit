//! Recovering chunk names from the `.bin` files of an archive.
//!
//! - The scan tells a bin from any other chunk by its magic, decoded from the
//!   first compressed block alone.
//! - The scan reads the strings of a bin as the length-prefixed runs the format
//!   writes, with no parse of the `PROP` data.
//!
//! # Example
//!
//! ```no_run
//! use std::collections::HashMap;
//! use std::fs::File;
//! use ltk_wad::{NameRecovery, Wad, WadExtractor, WadHash};
//!
//! let mut wad = Wad::mount(File::open("archive.wad.client")?)?;
//! let names: HashMap<WadHash, String> = HashMap::new();
//!
//! let recovered = NameRecovery::new().run(&mut wad, &names)?;
//! println!("{} names from {} bins", recovered.len(), recovered.bins_scanned);
//!
//! let layered = recovered.over(&names);
//! WadExtractor::new(&layered).extract_all(&mut wad, "/output/path")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::{Read, Seek},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, PoisonError,
    },
    thread,
};

use ltk_file::LeagueFileKind;
use ltk_hash::{Hash as _, WadHash};

use crate::{
    extractor::{default_workers, hex_name, lock},
    ChunkDecoder, PathResolver, Wad, WadChunk, WadError,
};

/// Raw bytes the sniff reads from a chunk first.
///
/// The first block of nearly every chunk fits, and a chunk whose block does not
/// gets a second read of [`SNIFF_RAW_BYTES`].
const SNIFF_FIRST_BYTES: usize = 16 * 1024;

/// Most raw bytes the sniff reads from one chunk.
///
/// A zstd block decodes to at most 128 KiB and an incompressible block is no
/// larger than that, so this always holds the first block and its headers.
const SNIFF_RAW_BYTES: usize = 256 * 1024;

/// Decoded bytes the sniff needs to tell a bin from anything else.
///
/// [`LeagueFileKind::identify_from_bytes`] reads eight at most.
const SNIFF_BYTES: usize = 8;

/// Shortest printable run worth a hash. No chunk path is shorter.
const MIN_CANDIDATE_LEN: usize = 4;

/// A chunk and its raw bytes, on the way from the reader to a worker.
type RawChunk = (WadChunk, Box<[u8]>);

/// Names recovered from the `.bin` files of one archive.
///
/// It is a [`PathResolver`] over the names it holds, and
/// [`over`](Self::over) layers it on top of another resolver.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecoveredNames {
    /// Path hash to the path a bin named it by, for chunks the resolver could not name.
    pub names: HashMap<WadHash, String>,
    /// Bins read for the names.
    pub bins_scanned: usize,
    /// Chunks whose first bytes the scan read because nothing named them.
    pub chunks_sniffed: usize,
}

impl RecoveredNames {
    /// The recovered path of a chunk, if a bin named it.
    pub fn get(&self, path_hash: WadHash) -> Option<&str> {
        self.names.get(&path_hash).map(String::as_str)
    }

    /// How many chunks gained a name.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether no chunk gained a name.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// A resolver that answers with a recovered name first and asks `fallback` for the rest.
    pub fn over<'a>(&'a self, fallback: &'a dyn PathResolver) -> LayeredResolver<'a> {
        LayeredResolver {
            names: self,
            fallback,
        }
    }
}

impl PathResolver for RecoveredNames {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        self.get(path_hash).map(String::from)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        self.names.contains_key(&path_hash)
    }
}

/// Recovered names over another resolver.
///
/// Built by [`RecoveredNames::over`].
pub struct LayeredResolver<'a> {
    names: &'a RecoveredNames,
    fallback: &'a dyn PathResolver,
}

impl fmt::Debug for LayeredResolver<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LayeredResolver")
            .field("names", &self.names)
            .finish_non_exhaustive()
    }
}

impl PathResolver for LayeredResolver<'_> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        match self.names.get(path_hash) {
            Some(name) => Some(String::from(name)),
            None => self.fallback.resolve(path_hash),
        }
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        self.names.is_known(path_hash) || self.fallback.is_known(path_hash)
    }
}

/// Configuration and execution of a name recovery over one archive.
#[derive(Debug, Clone, Default)]
pub struct NameRecovery<'a> {
    pub(crate) workers: Option<NonZeroUsize>,
    pub(crate) cancel: Option<&'a AtomicBool>,
}

impl<'a> NameRecovery<'a> {
    /// A recovery with the default worker count and no cancel flag.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set how many threads decompress and scan the bins.
    ///
    /// The default is the machine's available parallelism, capped at eight.
    pub fn with_workers(mut self, workers: NonZeroUsize) -> Self {
        self.workers = Some(workers);
        self
    }

    /// Stop early once `flag` reads `true`.
    ///
    /// [`run`](Self::run) returns the names found so far, and a stop is not an error.
    pub fn with_cancel_flag(mut self, flag: &'a AtomicBool) -> Self {
        self.cancel = Some(flag);
        self
    }

    /// Recover names for the chunks of `wad` that `resolver` cannot name.
    ///
    /// # Errors
    ///
    /// Fails when the recovery cannot read the archive, with a [`WadError::Chunk`]
    /// that names the bin. The recovery skips a bin that does not decompress,
    /// because one bad bin is no reason to lose the names in the rest.
    pub fn run<S: Read + Seek, R: PathResolver + ?Sized>(
        &self,
        wad: &mut Wad<S>,
        resolver: &R,
    ) -> Result<RecoveredNames, WadError> {
        let chunks: Vec<WadChunk> = wad.chunks().iter().copied().collect();

        let mut unknown = HashSet::new();
        let mut bins: Vec<(WadChunk, String)> = Vec::new();
        for chunk in &chunks {
            match resolver.resolve(chunk.path_hash) {
                Some(path) => {
                    if is_bin_name(&path) {
                        bins.push((*chunk, path));
                    }
                }
                None => {
                    unknown.insert(chunk.path_hash);
                }
            }
        }

        let mut recovered = RecoveredNames::default();
        if unknown.is_empty() {
            return Ok(recovered);
        }

        let mut decoder = ChunkDecoder::new();
        for chunk in &chunks {
            if !unknown.contains(&chunk.path_hash) {
                continue;
            }
            if self.cancelled() {
                return Ok(recovered);
            }
            recovered.chunks_sniffed += 1;
            if sniff_is_bin(wad, chunk, &mut decoder) {
                bins.push((*chunk, hex_name(chunk.path_hash)));
            }
        }
        if bins.is_empty() {
            return Ok(recovered);
        }

        let found = Mutex::new(HashMap::new());
        let workers = self.workers.map_or_else(default_workers, NonZeroUsize::get);
        let (sender, receiver) = mpsc::sync_channel::<RawChunk>(workers);
        let receiver = Mutex::new(receiver);

        /* Taken inside the scope so it drops before the scope joins the workers.
        A worker only stops once every sender is gone. */
        let mut sender = Some(sender);
        let scanned = thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| scan_bins(&receiver, &unknown, &found));
            }
            let sender = sender.take().expect("the sender is taken once");

            let mut scanned = 0;
            for (chunk, path) in &bins {
                if self.cancelled() || lock(&found).len() == unknown.len() {
                    break;
                }
                let raw = wad
                    .load_chunk_raw(chunk)
                    .map_err(|error| WadError::chunk(chunk.path_hash, path, error))?;
                scanned += 1;
                if sender.send((*chunk, raw)).is_err() {
                    break;
                }
            }
            Ok::<usize, WadError>(scanned)
        })?;

        recovered.bins_scanned = scanned;
        recovered.names = found.into_inner().unwrap_or_else(PoisonError::into_inner);
        Ok(recovered)
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed))
    }
}

/// Decompress each bin and claim every string in it that names an unknown chunk.
fn scan_bins(
    receiver: &Mutex<mpsc::Receiver<RawChunk>>,
    unknown: &HashSet<WadHash>,
    found: &Mutex<HashMap<WadHash, String>>,
) {
    let mut decoder = ChunkDecoder::new();
    loop {
        let Ok((chunk, raw)) = lock(receiver).recv() else {
            return;
        };
        let Ok(data) = decoder.decompress(&raw, chunk.compression_type, chunk.uncompressed_size)
        else {
            continue;
        };
        /* A chunk named `.bin` by a hash table is not always one. */
        if !is_bin_magic(&data) {
            continue;
        }

        for_each_candidate(&data, |candidate| {
            let hash = WadHash::hash_str(candidate);
            if unknown.contains(&hash) {
                lock(found)
                    .entry(hash)
                    .or_insert_with(|| candidate.to_owned());
            }
        });
    }
}

/// Whether a chunk's first bytes are a bin's, from as little of it as decodes them.
fn sniff_is_bin<S: Read + Seek>(
    wad: &mut Wad<S>,
    chunk: &WadChunk,
    decoder: &mut ChunkDecoder,
) -> bool {
    let wanted = SNIFF_BYTES.min(chunk.uncompressed_size);
    let mut limit = SNIFF_FIRST_BYTES;
    loop {
        let Ok(raw) = wad.load_chunk_raw_prefix(chunk, limit) else {
            return false;
        };
        match decoder.decompress_prefix(&raw, chunk.compression_type, SNIFF_BYTES) {
            Ok(head) if head.len() >= wanted => return is_bin_magic(&head),
            /* The prefix cut the first block short, and the chunk holds more. */
            _ if raw.len() == limit && limit < SNIFF_RAW_BYTES => limit = SNIFF_RAW_BYTES,
            _ => return false,
        }
    }
}

fn is_bin_magic(data: &[u8]) -> bool {
    matches!(
        LeagueFileKind::identify_from_bytes(data),
        LeagueFileKind::PropertyBin | LeagueFileKind::PropertyBinOverride
    )
}

fn is_bin_name(path: &str) -> bool {
    path.len() > 4
        && path.is_char_boundary(path.len() - 4)
        && path[path.len() - 4..].eq_ignore_ascii_case(".bin")
}

fn is_printable(byte: u8) -> bool {
    (0x20..=0x7e).contains(&byte)
}

/// Visit every string a bin holds, as the text between its length prefix and its end.
///
/// The format writes a string as a little-endian `u16` length and the bytes,
/// with no terminator. A path is printable ASCII and its length's high byte is
/// zero. So each run of printable bytes starts on a string's first byte, and
/// the two bytes in front of the run are its length. The length then bounds
/// the string exactly, however many printable bytes follow it, such as the low
/// byte of the next string's length.
fn for_each_candidate(data: &[u8], mut visit: impl FnMut(&str)) {
    let mut start = 0;
    while start < data.len() {
        if !is_printable(data[start]) {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < data.len() && is_printable(data[end]) {
            end += 1;
        }

        if start >= 2 && end - start >= MIN_CANDIDATE_LEN {
            let len = usize::from(u16::from_le_bytes([data[start - 2], data[start - 1]]));
            if len >= MIN_CANDIDATE_LEN && start + len <= end {
                let candidate = &data[start..start + len];
                if candidate.iter().any(|&byte| byte == b'/' || byte == b'.') {
                    /* Printable ASCII, so this cannot fail. */
                    if let Ok(candidate) = std::str::from_utf8(candidate) {
                        visit(candidate);
                    }
                }
            }
        }
        start = end;
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write as _};

    use camino::Utf8Path;

    use super::*;
    use crate::{ExtractLayout, NoResolver, WadBuilder, WadChunkBuilder, WadExtractor};

    fn collect_candidates(data: &[u8]) -> Vec<String> {
        let mut out = Vec::new();
        for_each_candidate(data, |candidate| out.push(candidate.to_owned()));
        out
    }

    /// A resolver that names the given paths, by their WAD hashes.
    fn names(paths: &[&str]) -> HashMap<WadHash, String> {
        paths
            .iter()
            .map(|path| (WadHash::hash_str(path), (*path).to_owned()))
            .collect()
    }

    /// A string as a bin writes it: a little-endian `u16` length and the bytes.
    fn bin_string(out: &mut Vec<u8>, text: &str) {
        out.extend_from_slice(&(text.len() as u16).to_le_bytes());
        out.extend_from_slice(text.as_bytes());
    }

    /// Enough of a bin to carry the magic and a few strings. The scan parses
    /// no structure, so the header is the magic, a version, and the strings.
    fn bin_with(paths: &[&str]) -> Vec<u8> {
        let mut out = b"PROP".to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(paths.len() as u32).to_le_bytes());
        for path in paths {
            bin_string(&mut out, path);
        }
        out.extend_from_slice(&[0x00, 0x01, 0x02, 0x03]);
        out
    }

    /// An archive of `(path, bytes)` chunks, as the builder compresses it.
    fn build_wad(chunks: &[(&str, Vec<u8>)]) -> Wad<Cursor<Vec<u8>>> {
        let mut builder = WadBuilder::default();
        for (path, _) in chunks {
            builder = builder.with_chunk(WadChunkBuilder::default().with_path(*path));
        }
        let mut buffer = Cursor::new(Vec::new());
        builder
            .build_to_writer(&mut buffer, |path_hash, cursor| {
                let (_, bytes) = chunks
                    .iter()
                    .find(|(path, _)| WadHash::hash_str(path) == path_hash)
                    .expect("every hash is one of the chunks");
                cursor.write_all(bytes)?;
                Ok(())
            })
            .unwrap();
        buffer.set_position(0);
        Wad::mount(buffer).unwrap()
    }

    #[test]
    fn candidates_are_the_length_prefixed_strings() {
        let mut data = vec![0xde, 0xad];
        bin_string(&mut data, "assets/foo.dds");
        data.extend_from_slice(&[0xff, 0xfe]);
        bin_string(&mut data, "a/b.bin");
        /* 32 bytes long, so the low byte of its length is a printable space
        that joins the run before it. The lengths still bound both. */
        bin_string(&mut data, "data/characters/aatrox/x.anm_ext");
        data.extend_from_slice(&[0x00, 0x00]);
        bin_string(&mut data, "noise");

        assert_eq!(
            collect_candidates(&data),
            vec![
                "assets/foo.dds".to_string(),
                "a/b.bin".to_string(),
                "data/characters/aatrox/x.anm_ext".to_string(),
            ]
        );
    }

    #[test]
    fn nothing_runs_when_every_chunk_is_named() {
        let mut wad = build_wad(&[
            ("data/test.bin", bin_with(&["assets/a.dds"])),
            ("assets/a.dds", vec![0xab; 16]),
        ]);
        let resolver = names(&["data/test.bin", "assets/a.dds"]);

        let recovered = NameRecovery::new().run(&mut wad, &resolver).unwrap();

        assert_eq!(recovered, RecoveredNames::default());
    }

    #[test]
    fn a_named_bin_names_the_chunks_it_links() {
        let asset = "ASSETS/Characters/Foo/Recovered.dds";
        let mut wad = build_wad(&[
            ("data/test.bin", bin_with(&[asset])),
            (asset, vec![0xab; 32]),
        ]);
        let resolver = names(&["data/test.bin"]);

        let recovered = NameRecovery::new().run(&mut wad, &resolver).unwrap();

        assert_eq!(recovered.get(WadHash::hash_str(asset)), Some(asset));
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered.bins_scanned, 1);
        /* The recovery sniffed the asset, and not the named bin. */
        assert_eq!(recovered.chunks_sniffed, 1);
        assert!(!recovered
            .names
            .contains_key(&WadHash::hash_str("data/test.bin")));
    }

    #[test]
    fn an_anonymous_bin_is_found_by_its_first_bytes() {
        let asset = "assets/orphan/asset.dds";
        let mut wad = build_wad(&[
            ("data/orphan.bin", bin_with(&[asset])),
            (asset, vec![0xee; 16]),
        ]);

        let recovered = NameRecovery::new().run(&mut wad, &NoResolver).unwrap();

        assert_eq!(recovered.get(WadHash::hash_str(asset)), Some(asset));
        assert_eq!(recovered.chunks_sniffed, 2);
        assert_eq!(recovered.bins_scanned, 1);
    }

    #[test]
    fn a_set_cancel_flag_returns_what_was_found() {
        let mut wad = build_wad(&[
            ("data/test.bin", bin_with(&["assets/a.dds"])),
            ("assets/a.dds", vec![0xab; 16]),
        ]);
        let flag = AtomicBool::new(true);

        let recovered = NameRecovery::new()
            .with_cancel_flag(&flag)
            .run(&mut wad, &NoResolver)
            .unwrap();

        assert!(recovered.is_empty());
        assert_eq!(recovered.bins_scanned, 0);
    }

    #[test]
    fn the_layered_resolver_reads_recovered_names_first() {
        let (found, known, neither) = (WadHash(0x1234), WadHash(0x5678), WadHash(0x9abc));
        let mut recovered = RecoveredNames::default();
        recovered.names.insert(found, "assets/found.dds".into());
        let mut fallback: HashMap<WadHash, String> = HashMap::new();
        fallback.insert(known, "assets/known.dds".into());

        assert_eq!(
            recovered.resolve(found).as_deref(),
            Some("assets/found.dds")
        );
        assert_eq!(recovered.resolve(known), None);

        let layered = recovered.over(&fallback);

        assert_eq!(layered.resolve(found).as_deref(), Some("assets/found.dds"));
        assert_eq!(layered.resolve(known).as_deref(), Some("assets/known.dds"));
        assert_eq!(layered.resolve(neither), None);
        assert!(layered.is_known(found));
        assert!(layered.is_known(known));
        assert!(!layered.is_known(neither));
    }

    #[test]
    fn the_extractor_writes_a_recovered_name() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let asset = "assets/characters/foo/recovered.dds";
        let mut wad = build_wad(&[
            ("data/test.bin", bin_with(&[asset])),
            (asset, vec![0xab; 32]),
        ]);
        let resolver = names(&["data/test.bin"]);

        let report = WadExtractor::new(&resolver)
            .with_name_recovery()
            .with_layout(ExtractLayout::Paths)
            .extract_all(&mut wad, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2);
        assert_eq!(report.recovered.len(), 1);
        assert_eq!(report.recovered.bins_scanned, 1);
        assert!(temp_dir.path().join(asset).exists());
        assert!(temp_dir.path().join("data/test.bin").exists());
    }
}
