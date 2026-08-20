//! WAD chunk extraction utilities.
//!
//! This module provides abstractions for extracting chunks from WAD archives to disk.
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use std::borrow::Cow;
//! use std::collections::HashMap;
//! use ltk_wad::{Wad, PathResolver, WadExtractor, ExtractProgress};
//!
//! // Implement your own path resolver (e.g., from a hashtable file)
//! struct MyHashtable {
//!     paths: HashMap<u64, String>,
//! }
//!
//! impl PathResolver for MyHashtable {
//!     fn resolve(&self, path_hash: u64) -> Cow<'_, str> {
//!         self.paths
//!             .get(&path_hash)
//!             .map(|s| Cow::Borrowed(s.as_str()))
//!             .unwrap_or_else(|| Cow::Owned(format!("{:016x}", path_hash)))
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let file = File::open("archive.wad.client")?;
//!     let mut wad = Wad::mount(file)?;
//!     let hashtable = MyHashtable { paths: HashMap::new() };
//!
//!     // Build the extractor with a progress callback
//!     let extractor = WadExtractor::new(&hashtable)
//!         .on_progress(|progress| {
//!             println!("Progress: {:.1}% - {}", progress.percent() * 100.0, progress.current_path());
//!         });
//!
//!     let report = extractor.extract_all(&mut wad, "/output/path")?;
//!     println!("Extracted {} chunks ({} bytes)", report.extracted, report.bytes_written);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Extracting a selection
//!
//! [`WadExtractor::extract_chunks`] takes any slice of the archive's chunks, so a
//! caller that knows which path hashes it wants extracts those alone:
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_wad::{Wad, WadExtractor, HexPathResolver, ExtractLayout, ExistingFilePolicy};
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//!
//! let wanted = [0x1234567890abcdef_u64, 0xfedcba0987654321];
//! let chunks: Vec<_> = wanted
//!     .iter()
//!     .filter_map(|hash| wad.chunks().get(*hash).copied())
//!     .collect();
//!
//! let extractor = WadExtractor::new(&HexPathResolver)
//!     .with_layout(ExtractLayout::Flat)
//!     .with_existing_file_policy(ExistingFilePolicy::Skip);
//! let report = extractor.extract_chunks(&mut wad, &chunks, "/output/path")?;
//! println!("{} written, {} were there already", report.extracted, report.skipped_existing);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # How a chunk is named on disk
//!
//! - A path the resolver knows is used as is.
//! - A path that is a bare hash (16 hex digits) gains the extension its bytes
//!   identify as, when they identify as anything.
//! - A path with no extension, or one that collides with an existing directory,
//!   becomes `<stem>.ltk.<ext>`, or `<stem>.ltk` when the bytes identify as nothing.
//! - A name the file system refuses as too long falls back to `<hash>.<ext>` in
//!   the output directory itself.
//!
//! A chunk no resolver names can still get its name from the archive itself.
//! [`WadExtractor::with_name_recovery`] reads the `.bin` files for it first.
//! Read [`NameRecovery`] for how.
//!
//! # Parallelism
//!
//! [`extract_all`](WadExtractor::extract_all) and
//! [`extract_chunks`](WadExtractor::extract_chunks) read the archive in order on
//! the calling thread and hand each chunk to a worker that decompresses and
//! writes it. The channel between the two is bounded by the worker count, so
//! memory holds a few chunks whatever the archive holds. The resolver, the path
//! filter and the progress callback run on the calling thread only, so none of
//! them needs to be [`Sync`].

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fs::{self, OpenOptions},
    io::{self, Read, Seek, Write as _},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, MutexGuard, PoisonError,
    },
    thread,
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;

use crate::{ChunkDecoder, NameRecovery, RecoveredNames, Wad, WadChunk, WadError};

/// A trait for resolving path hashes to human-readable paths.
///
/// Implement this trait to provide path resolution from a hashtable or other source.
pub trait PathResolver {
    /// Resolve a path hash to a path string.
    ///
    /// If the hash cannot be resolved, implementations should return the hash
    /// formatted as a hex string (e.g., `format!("{:016x}", path_hash)`).
    fn resolve(&self, path_hash: u64) -> Cow<'_, str>;

    /// Whether the resolver names this hash, rather than falling back to the hex string.
    ///
    /// The default reads [`resolve`](Self::resolve) and reports a name that is
    /// not sixteen hex digits. A resolver that can answer without building the
    /// string should override it.
    fn is_known(&self, path_hash: u64) -> bool {
        !is_hex_chunk_path(Utf8Path::new(self.resolve(path_hash).as_ref()))
    }
}

/// A trait for filtering chunks by path pattern.
///
/// Implement this trait to provide custom pattern matching logic.
pub trait PathFilter {
    /// Returns `true` if the path matches the filter pattern.
    fn matches(&self, path: &str) -> bool;
}

/// A path resolver that simply returns the hash as a hex string.
///
/// Useful when no hashtable is available.
#[derive(Debug, Clone, Copy, Default)]
pub struct HexPathResolver;

impl PathResolver for HexPathResolver {
    fn resolve(&self, path_hash: u64) -> Cow<'_, str> {
        Cow::Owned(format!("{:016x}", path_hash))
    }

    fn is_known(&self, _path_hash: u64) -> bool {
        false
    }
}

/// A path resolver backed by a `HashMap<u64, String>`.
#[derive(Debug, Clone, Default)]
pub struct HashMapPathResolver {
    paths: HashMap<u64, String>,
}

impl HashMapPathResolver {
    /// Create a new resolver with the given path mappings.
    pub fn new(paths: HashMap<u64, String>) -> Self {
        Self { paths }
    }

    /// Insert a path mapping.
    pub fn insert(&mut self, hash: u64, path: String) {
        self.paths.insert(hash, path);
    }

    /// Get a reference to the inner map.
    pub fn inner(&self) -> &HashMap<u64, String> {
        &self.paths
    }

    /// Get a mutable reference to the inner map.
    pub fn inner_mut(&mut self) -> &mut HashMap<u64, String> {
        &mut self.paths
    }
}

impl PathResolver for HashMapPathResolver {
    fn resolve(&self, path_hash: u64) -> Cow<'_, str> {
        self.paths
            .get(&path_hash)
            .map(|s| Cow::Borrowed(s.as_str()))
            .unwrap_or_else(|| Cow::Owned(format!("{:016x}", path_hash)))
    }

    fn is_known(&self, path_hash: u64) -> bool {
        self.paths.contains_key(&path_hash)
    }
}

impl From<HashMap<u64, String>> for HashMapPathResolver {
    fn from(paths: HashMap<u64, String>) -> Self {
        Self::new(paths)
    }
}

/// Information about extraction progress.
#[derive(Debug, Clone)]
pub struct ExtractProgress<'a> {
    /// Current chunk index (0-based).
    pub current: usize,
    /// Total number of chunks.
    pub total: usize,
    /// Path of the current chunk being processed.
    pub current_path: &'a str,
    /// Path hash of the current chunk.
    pub path_hash: u64,
}

impl ExtractProgress<'_> {
    /// Progress as a fraction from 0.0 to 1.0.
    #[inline]
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.current as f64 / self.total as f64
        }
    }

    /// Get the current path being processed.
    #[inline]
    pub fn current_path(&self) -> &str {
        self.current_path
    }
}

/// Result of a single chunk extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtractResult {
    /// The chunk was extracted successfully.
    Extracted,
    /// The chunk was skipped due to type filtering.
    SkippedByType,
    /// The chunk was skipped due to pattern filtering.
    SkippedByPattern,
    /// The chunk's file existed already, and the policy was [`ExistingFilePolicy::Skip`].
    SkippedExisting,
}

/// Where each extracted chunk lands under the output directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExtractLayout {
    /// At its resolved path, with every directory of that path.
    #[default]
    Paths,
    /// In the output directory itself, by its file name alone.
    ///
    /// When two chunks of one extraction share a name, the second keeps apart
    /// with its path hash before the extension, as `name.<hash>.ext`. Which of
    /// the two is second follows write order, so build one extractor per
    /// extraction when driving [`WadExtractor::extract_chunk_data`] yourself.
    Flat,
}

/// What to do with a chunk whose file exists already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExistingFilePolicy {
    /// Write over it.
    #[default]
    Overwrite,
    /// Leave it, and count the chunk under [`ExtractReport::skipped_existing`].
    ///
    /// The file is opened with `create_new`, so a file that appears between two
    /// chunks is left alone too and no check races the write.
    Skip,
}

/// What an extraction did, summed over its chunks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExtractReport {
    /// Chunks written to disk.
    pub extracted: usize,
    /// Chunks left alone under [`ExistingFilePolicy::Skip`].
    pub skipped_existing: usize,
    /// Chunks the path filter or the type filter left out.
    pub skipped_by_filter: usize,
    /// Bytes written, after decompression.
    pub bytes_written: u64,
    /// Written chunks, by the kind their bytes identify as.
    pub by_kind: BTreeMap<LeagueFileKind, usize>,
    /// The cancel flag was set, so some chunks were never read.
    pub cancelled: bool,
    /// Names that [`with_name_recovery`](WadExtractor::with_name_recovery) read
    /// out of the archive's bins, by path hash. Empty when recovery was off.
    pub recovered_names: HashMap<u64, String>,
}

impl ExtractReport {
    fn record(&mut self, outcome: ChunkOutcome) {
        match outcome {
            ChunkOutcome::Written { kind, bytes } => {
                self.extracted += 1;
                self.bytes_written += bytes;
                *self.by_kind.entry(kind).or_insert(0) += 1;
            }
            ChunkOutcome::SkippedByType => self.skipped_by_filter += 1,
            ChunkOutcome::SkippedExisting => self.skipped_existing += 1,
        }
    }
}

/// What happened to one chunk, with the figures the report sums.
#[derive(Debug, Clone, Copy)]
enum ChunkOutcome {
    Written { kind: LeagueFileKind, bytes: u64 },
    SkippedByType,
    SkippedExisting,
}

impl From<ChunkOutcome> for ExtractResult {
    fn from(outcome: ChunkOutcome) -> Self {
        match outcome {
            ChunkOutcome::Written { .. } => ExtractResult::Extracted,
            ChunkOutcome::SkippedByType => ExtractResult::SkippedByType,
            ChunkOutcome::SkippedExisting => ExtractResult::SkippedExisting,
        }
    }
}

/// Type alias for the progress callback function.
pub type ProgressCallback<'a> = Box<dyn Fn(ExtractProgress<'_>) + 'a>;

/// Most workers an extraction starts unless [`WadExtractor::with_workers`] says
/// otherwise. Each worker holds a compressed and a decompressed chunk at once,
/// and a wide machine would otherwise hold dozens of the largest ones.
const DEFAULT_WORKER_CAP: usize = 8;

/// Configuration and execution of WAD chunk extraction.
///
/// # Type Parameters
///
/// * `R` - The path resolver type
/// * `F` - The path filter type (optional)
pub struct WadExtractor<'a, R: PathResolver, F: PathFilter = NoFilter> {
    resolver: &'a R,
    filter: Option<F>,
    type_filter: Option<Vec<LeagueFileKind>>,
    progress_callback: Option<ProgressCallback<'a>>,
    layout: ExtractLayout,
    existing: ExistingFilePolicy,
    cancel: Option<&'a AtomicBool>,
    workers: Option<NonZeroUsize>,
    recover_names: bool,
    /* The names the flat layout has handed out, so a second chunk of one name
    can tell. Behind a mutex because the workers claim names concurrently. */
    flat_names: Mutex<HashSet<String>>,
}

/// A filter that matches all paths (no filtering).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFilter;

impl PathFilter for NoFilter {
    fn matches(&self, _path: &str) -> bool {
        true
    }
}

impl<'a, R: PathResolver> WadExtractor<'a, R, NoFilter> {
    /// Create a new extractor with the given path resolver.
    pub fn new(resolver: &'a R) -> Self {
        Self {
            resolver,
            filter: None,
            type_filter: None,
            progress_callback: None,
            layout: ExtractLayout::default(),
            existing: ExistingFilePolicy::default(),
            cancel: None,
            workers: None,
            recover_names: false,
            flat_names: Mutex::default(),
        }
    }
}

impl<'a, R: PathResolver, F: PathFilter> WadExtractor<'a, R, F> {
    /// Set a path filter for the extractor.
    ///
    /// Only chunks whose paths match the filter will be extracted.
    pub fn with_filter<F2: PathFilter>(self, filter: F2) -> WadExtractor<'a, R, F2> {
        WadExtractor {
            resolver: self.resolver,
            filter: Some(filter),
            type_filter: self.type_filter,
            progress_callback: self.progress_callback,
            layout: self.layout,
            existing: self.existing,
            cancel: self.cancel,
            workers: self.workers,
            recover_names: self.recover_names,
            flat_names: self.flat_names,
        }
    }

    /// Set a type filter for the extractor.
    ///
    /// Only chunks whose detected file type is in the list will be extracted.
    pub fn with_type_filter(mut self, types: Vec<LeagueFileKind>) -> Self {
        self.type_filter = Some(types);
        self
    }

    /// Set a progress callback.
    ///
    /// The callback will be invoked for each chunk processed (including skipped chunks).
    pub fn on_progress<C: Fn(ExtractProgress<'_>) + 'a>(mut self, callback: C) -> Self {
        self.progress_callback = Some(Box::new(callback));
        self
    }

    /// Set where the extracted chunks land.
    ///
    /// The default is [`ExtractLayout::Paths`].
    pub fn with_layout(mut self, layout: ExtractLayout) -> Self {
        self.layout = layout;
        self
    }

    /// Set what happens to a chunk whose file exists already.
    ///
    /// The default is [`ExistingFilePolicy::Overwrite`].
    pub fn with_existing_file_policy(mut self, policy: ExistingFilePolicy) -> Self {
        self.existing = policy;
        self
    }

    /// Stop early once `flag` reads `true`.
    ///
    /// The flag is read before each chunk is read from the archive. Chunks
    /// already handed to a worker still land, and
    /// [`ExtractReport::cancelled`] reports the stop.
    pub fn with_cancel_flag(mut self, flag: &'a AtomicBool) -> Self {
        self.cancel = Some(flag);
        self
    }

    /// Set how many threads decompress and write.
    ///
    /// The default is the machine's available parallelism, capped at eight.
    pub fn with_workers(mut self, workers: NonZeroUsize) -> Self {
        self.workers = Some(workers);
        self
    }

    /// Read the archive's `.bin` files for the names of chunks the resolver
    /// cannot name, before anything is written.
    ///
    /// Read [`NameRecovery`] for what it costs and what it finds. The names
    /// land in [`ExtractReport::recovered_names`], and the extraction uses the
    /// same workers and cancel flag for the read.
    pub fn with_name_recovery(mut self) -> Self {
        self.recover_names = true;
        self
    }

    /// Extract every chunk of `wad` into `output_dir`.
    ///
    /// # Errors
    ///
    /// Fails on the first chunk that cannot be read, decompressed or written.
    /// Chunks written before it stay on disk.
    pub fn extract_all<TSource: Read + Seek>(
        &self,
        wad: &mut Wad<TSource>,
        output_dir: impl AsRef<Utf8Path>,
    ) -> Result<ExtractReport, WadError> {
        let chunks: Vec<WadChunk> = wad.chunks().iter().copied().collect();
        self.extract_chunks(wad, &chunks, output_dir)
    }

    /// Extract `chunks` of `wad` into `output_dir`, in the order given.
    ///
    /// A chunk is read at the offsets it carries, so take the chunks from
    /// [`Wad::chunks`] of the same archive.
    ///
    /// # Errors
    ///
    /// Fails on the first chunk that cannot be read, decompressed or written.
    /// Chunks written before it stay on disk.
    pub fn extract_chunks<TSource: Read + Seek>(
        &self,
        wad: &mut Wad<TSource>,
        chunks: &[WadChunk],
        output_dir: impl AsRef<Utf8Path>,
    ) -> Result<ExtractReport, WadError> {
        let output_dir = output_dir.as_ref();
        let workers = self.workers.map_or_else(default_workers, NonZeroUsize::get);

        let recovered = if self.recover_names {
            NameRecovery {
                workers: self.workers,
                cancel: self.cancel,
            }
            .run(wad, self.resolver)?
        } else {
            RecoveredNames::default()
        };
        let resolver = recovered.over(self.resolver);

        let shared = Shared {
            writer: self.writer(output_dir),
            report: Mutex::default(),
            failure: Mutex::default(),
        };
        let (sender, receiver) = mpsc::sync_channel::<Job>(workers);
        let receiver = Mutex::new(receiver);

        /* Taken inside the scope so it drops before the workers are joined.
        A worker only stops once every sender is gone. */
        let mut sender = Some(sender);
        let cancelled = thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| shared.run_worker(&receiver));
            }
            let sender = sender.take().expect("the sender is taken once");
            self.read_chunks(wad, chunks, &sender, &shared, &resolver)
        })?;

        if let Some(error) = shared
            .failure
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner)
        {
            return Err(error);
        }
        let mut report = shared
            .report
            .into_inner()
            .unwrap_or_else(PoisonError::into_inner);
        report.cancelled = cancelled;
        report.recovered_names = recovered.names;
        Ok(report)
    }

    /// Extract a single chunk from already-decompressed data to the specified directory.
    ///
    /// This is useful for parallel workflows where decompression is done separately
    /// (e.g. via [`decompress_raw`](crate::decompress_raw)). The layout and the
    /// existing-file policy apply as they do to [`extract_chunks`](Self::extract_chunks).
    pub fn extract_chunk_data(
        &self,
        chunk: &WadChunk,
        chunk_data: &[u8],
        chunk_path: &Utf8Path,
        output_dir: &Utf8Path,
    ) -> Result<ExtractResult, WadError> {
        self.writer(output_dir)
            .write_chunk(chunk, chunk_data, chunk_path)
            .map(ExtractResult::from)
    }

    fn writer<'s>(&'s self, output_dir: &'s Utf8Path) -> ChunkWriter<'s> {
        ChunkWriter {
            layout: self.layout,
            existing: self.existing,
            type_filter: self.type_filter.as_deref(),
            output_dir,
            flat_names: &self.flat_names,
        }
    }

    /// Feed the workers, chunk by chunk, and say whether the cancel flag stopped it.
    fn read_chunks<TSource: Read + Seek>(
        &self,
        wad: &mut Wad<TSource>,
        chunks: &[WadChunk],
        sender: &mpsc::SyncSender<Job>,
        shared: &Shared<'_>,
        resolver: &impl PathResolver,
    ) -> Result<bool, WadError> {
        let total = chunks.len();
        for (index, chunk) in chunks.iter().enumerate() {
            if self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Ok(true);
            }
            if shared.failed() {
                return Ok(false);
            }

            let chunk_path = resolver.resolve(chunk.path_hash);

            if let Some(callback) = &self.progress_callback {
                callback(ExtractProgress {
                    current: index,
                    total,
                    current_path: chunk_path.as_ref(),
                    path_hash: chunk.path_hash,
                });
            }

            if let Some(filter) = &self.filter {
                if !filter.matches(chunk_path.as_ref()) {
                    lock(&shared.report).skipped_by_filter += 1;
                    continue;
                }
            }

            let raw = wad.load_chunk_raw(chunk)?;
            let job = Job {
                chunk: *chunk,
                path: chunk_path.into_owned(),
                raw,
            };
            /* Refused only once every worker is gone, which takes a panic. */
            if sender.send(job).is_err() {
                break;
            }
        }
        Ok(false)
    }
}

pub(crate) fn default_workers() -> usize {
    thread::available_parallelism().map_or(1, |count| count.get().min(DEFAULT_WORKER_CAP))
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One chunk on its way from the reader to a worker.
struct Job {
    chunk: WadChunk,
    path: String,
    raw: Box<[u8]>,
}

/// What the workers share: the writer, the tally, and the first failure.
struct Shared<'s> {
    writer: ChunkWriter<'s>,
    report: Mutex<ExtractReport>,
    failure: Mutex<Option<WadError>>,
}

impl Shared<'_> {
    fn failed(&self) -> bool {
        lock(&self.failure).is_some()
    }

    /// Take jobs until every sender is gone.
    ///
    /// After a failure the jobs are drained and dropped rather than written,
    /// so a reader blocked on a full channel gets to see the failure too.
    fn run_worker(&self, receiver: &Mutex<mpsc::Receiver<Job>>) {
        let mut decoder = ChunkDecoder::new();
        loop {
            let Ok(job) = lock(receiver).recv() else {
                return;
            };
            if self.failed() {
                continue;
            }
            match self.writer.write(&job, &mut decoder) {
                Ok(outcome) => lock(&self.report).record(outcome),
                Err(error) => {
                    let mut failure = lock(&self.failure);
                    if failure.is_none() {
                        *failure = Some(error);
                    }
                }
            }
        }
    }
}

/// The half of the extractor that the workers share.
///
/// Everything here is [`Sync`]. The resolver, the path filter and the progress
/// callback stay behind on the reader, which is what keeps those three free of
/// any such bound.
struct ChunkWriter<'s> {
    layout: ExtractLayout,
    existing: ExistingFilePolicy,
    type_filter: Option<&'s [LeagueFileKind]>,
    output_dir: &'s Utf8Path,
    flat_names: &'s Mutex<HashSet<String>>,
}

impl ChunkWriter<'_> {
    fn write(&self, job: &Job, decoder: &mut ChunkDecoder) -> Result<ChunkOutcome, WadError> {
        let data = decoder.decompress(
            &job.raw,
            job.chunk.compression_type,
            job.chunk.uncompressed_size,
        )?;
        self.write_chunk(&job.chunk, &data, Utf8Path::new(&job.path))
    }

    fn write_chunk(
        &self,
        chunk: &WadChunk,
        chunk_data: &[u8],
        chunk_path: &Utf8Path,
    ) -> Result<ChunkOutcome, WadError> {
        let chunk_kind = LeagueFileKind::identify_from_bytes(chunk_data);

        if self
            .type_filter
            .is_some_and(|types| !types.contains(&chunk_kind))
        {
            return Ok(ChunkOutcome::SkippedByType);
        }

        let relative_path = match self.layout {
            ExtractLayout::Paths => self.resolve_final_path(chunk_path, chunk_data, chunk_kind),
            ExtractLayout::Flat => {
                self.resolve_flat_path(chunk, chunk_path, chunk_data, chunk_kind)
            }
        };
        let full_path = self.output_dir.join(&relative_path);

        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let written = match write_file(&full_path, chunk_data, self.existing) {
            Ok(written) => written,
            Err(error) if error.kind() == io::ErrorKind::InvalidFilename => {
                let hashed_path = self.output_dir.join(hashed_name(chunk, chunk_kind));
                write_file(&hashed_path, chunk_data, self.existing)?
            }
            Err(error) => return Err(WadError::IoError(error)),
        };

        Ok(match written {
            Written::Yes => ChunkOutcome::Written {
                kind: chunk_kind,
                bytes: chunk_data.len() as u64,
            },
            Written::Existed => ChunkOutcome::SkippedExisting,
        })
    }

    /// Resolve the final output path for a chunk.
    fn resolve_final_path(
        &self,
        chunk_path: &Utf8Path,
        chunk_data: &[u8],
        chunk_kind: LeagueFileKind,
    ) -> Utf8PathBuf {
        let mut final_path = chunk_path.to_path_buf();

        // If the path looks like a hex hash (no extension), add the detected extension
        if is_hex_chunk_path(&final_path) {
            if let Some(ext) = chunk_kind.extension() {
                final_path.set_extension(ext);
            }
            return final_path;
        }

        // - If the original path has no extension, affix .ltk (and real extension if known)
        // - OR if the destination path collides with an existing directory, affix .ltk
        let has_extension = final_path.extension().is_some();
        let collides_with_dir = self.output_dir.join(&final_path).is_dir();
        if !has_extension || collides_with_dir {
            final_path.set_file_name(build_ltk_name(
                chunk_path.file_stem().unwrap_or_default(),
                chunk_data,
            ));
        }

        final_path
    }

    /// The file name alone, made unique among the names this extraction wrote.
    fn resolve_flat_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        chunk_data: &[u8],
        chunk_kind: LeagueFileKind,
    ) -> Utf8PathBuf {
        let file_name = Utf8Path::new(chunk_path.file_name().unwrap_or_default());
        let named = self.resolve_final_path(file_name, chunk_data, chunk_kind);

        let mut names = lock(self.flat_names);
        if names.insert(named.as_str().to_owned()) {
            return named;
        }

        let suffixed = match named.extension() {
            Some(ext) => format!(
                "{}.{:016x}.{ext}",
                named.file_stem().unwrap_or_default(),
                chunk.path_hash
            ),
            None => format!("{}.{:016x}", named.as_str(), chunk.path_hash),
        };
        names.insert(suffixed.clone());
        Utf8PathBuf::from(suffixed)
    }
}

enum Written {
    Yes,
    Existed,
}

/// Write `data` to `path` under `policy`.
///
/// `Skip` opens with `create_new`, which makes the existence check and the
/// create one operation, so two workers can never both write one path.
fn write_file(path: &Utf8Path, data: &[u8], policy: ExistingFilePolicy) -> io::Result<Written> {
    match policy {
        ExistingFilePolicy::Overwrite => {
            fs::write(path, data)?;
            Ok(Written::Yes)
        }
        ExistingFilePolicy::Skip => {
            match OpenOptions::new().write(true).create_new(true).open(path) {
                Ok(mut file) => {
                    file.write_all(data)?;
                    Ok(Written::Yes)
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(Written::Existed),
                Err(error) => Err(error),
            }
        }
    }
}

/// `<hash>.<ext>`, the name a chunk falls back to when its own is refused.
fn hashed_name(chunk: &WadChunk, chunk_kind: LeagueFileKind) -> Utf8PathBuf {
    let mut hashed_path = Utf8PathBuf::from(format!("{:016x}", chunk.path_hash));
    if let Some(ext) = chunk_kind.extension() {
        hashed_path.set_extension(ext);
    }
    hashed_path
}

/// Check if a path looks like a hex-encoded hash (e.g., "0123456789abcdef").
///
/// This is useful for determining if a chunk path is unresolved (just a hash)
/// or if it has been resolved to a human-readable path.
///
/// # Example
///
/// ```
/// use ltk_wad::is_hex_chunk_path;
/// use camino::Utf8Path;
///
/// assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef")));
/// assert!(is_hex_chunk_path(Utf8Path::new("0123456789abcdef.bin")));
/// assert!(!is_hex_chunk_path(Utf8Path::new("assets/champions/aatrox.bin")));
/// ```
pub fn is_hex_chunk_path(path: &Utf8Path) -> bool {
    let file_stem = path.file_stem().unwrap_or("");
    file_stem.len() == 16 && file_stem.chars().all(|c| c.is_ascii_hexdigit())
}

/// Build a filename with `.ltk` suffix and optional type extension.
fn build_ltk_name(file_stem: impl AsRef<str>, chunk_data: &[u8]) -> String {
    let kind = LeagueFileKind::identify_from_bytes(chunk_data);
    match kind.extension() {
        Some(ext) => format!("{}.ltk.{}", file_stem.as_ref(), ext),
        None => format!("{}.ltk", file_stem.as_ref()),
    }
}

#[cfg(feature = "regex")]
mod regex_filter {
    use super::PathFilter;

    /// A path filter using a regular expression.
    #[derive(Debug, Clone)]
    pub struct RegexFilter {
        pattern: regex::Regex,
    }

    impl RegexFilter {
        /// Create a new regex filter from a pattern string.
        ///
        /// Returns `None` if the pattern is invalid.
        pub fn new(pattern: &str) -> Option<Self> {
            regex::Regex::new(pattern)
                .ok()
                .map(|pattern| Self { pattern })
        }

        /// Create a new regex filter from a compiled regex.
        pub fn from_regex(pattern: regex::Regex) -> Self {
            Self { pattern }
        }
    }

    impl PathFilter for RegexFilter {
        fn matches(&self, path: &str) -> bool {
            self.pattern.is_match(path)
        }
    }
}

#[cfg(feature = "regex")]
pub use regex_filter::RegexFilter;

#[cfg(test)]
mod tests {
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
            path_hash,
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
            path_hash,
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

    /// Custom path filter for testing.
    struct PrefixFilter {
        prefix: String,
    }

    impl PrefixFilter {
        fn new(prefix: impl Into<String>) -> Self {
            Self {
                prefix: prefix.into(),
            }
        }
    }

    impl PathFilter for PrefixFilter {
        fn matches(&self, path: &str) -> bool {
            path.starts_with(&self.prefix)
        }
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
    fn test_hex_path_resolver() {
        let resolver = HexPathResolver;
        assert_eq!(resolver.resolve(0x0123456789abcdef), "0123456789abcdef");
    }

    #[test]
    fn test_hex_path_resolver_formats_hash_correctly() {
        let resolver = HexPathResolver;

        // Test various hashes
        assert_eq!(resolver.resolve(0x0), "0000000000000000");
        assert_eq!(resolver.resolve(0x1), "0000000000000001");
        assert_eq!(resolver.resolve(0x123456789abcdef0), "123456789abcdef0");
        assert_eq!(resolver.resolve(u64::MAX), "ffffffffffffffff");
    }

    #[test]
    fn test_hashmap_path_resolver() {
        let mut resolver = HashMapPathResolver::new(HashMap::new());
        resolver.insert(0x1234, "assets/test.bin".to_string());

        assert_eq!(resolver.resolve(0x1234), "assets/test.bin");
        assert_eq!(resolver.resolve(0x5678), "0000000000005678");
    }

    #[test]
    fn test_hashmap_path_resolver_resolves_known_paths() {
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234, "assets/champions/aatrox.bin".to_string());
        resolver.insert(0x5678, "data/maps/summoners_rift.mapgeo".to_string());

        assert_eq!(resolver.resolve(0x1234), "assets/champions/aatrox.bin");
        assert_eq!(resolver.resolve(0x5678), "data/maps/summoners_rift.mapgeo");
    }

    #[test]
    fn test_hashmap_path_resolver_falls_back_to_hex() {
        let resolver = HashMapPathResolver::new(HashMap::new());

        // Unknown hashes should return hex format
        assert_eq!(resolver.resolve(0xdeadbeef), "00000000deadbeef");
        assert_eq!(resolver.resolve(0x1234567890abcdef), "1234567890abcdef");
    }

    #[test]
    fn test_hashmap_path_resolver_from_hashmap() {
        let mut paths = HashMap::new();
        paths.insert(0xabc, "test/path.bin".to_string());

        let resolver: HashMapPathResolver = paths.into();
        assert_eq!(resolver.resolve(0xabc), "test/path.bin");
    }

    #[test]
    fn test_hashmap_path_resolver_inner_access() {
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1, "one".to_string());

        // Test inner() access
        assert_eq!(resolver.inner().get(&0x1), Some(&"one".to_string()));

        // Test inner_mut() access
        resolver.inner_mut().insert(0x2, "two".to_string());
        assert_eq!(resolver.resolve(0x2), "two");
    }

    // =============================================================================
    // PathFilter Tests
    // =============================================================================

    #[test]
    fn test_no_filter_matches_everything() {
        let filter = NoFilter;

        assert!(filter.matches(""));
        assert!(filter.matches("any/path/here.bin"));
        assert!(filter.matches("0123456789abcdef"));
    }

    #[test]
    fn test_custom_prefix_filter() {
        let filter = PrefixFilter::new("assets/");

        assert!(filter.matches("assets/champions/aatrox.bin"));
        assert!(filter.matches("assets/maps/test.mapgeo"));
        assert!(!filter.matches("data/test.bin"));
        assert!(!filter.matches(""));
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

    #[test]
    fn test_extract_progress() {
        let progress = ExtractProgress {
            current: 50,
            total: 100,
            current_path: "test/path.bin",
            path_hash: 0x1234,
        };

        assert!((progress.percent() - 0.5).abs() < f64::EPSILON);
        assert_eq!(progress.current_path(), "test/path.bin");
    }

    #[test]
    fn test_extract_progress_at_boundaries() {
        // Start
        let start = ExtractProgress {
            current: 0,
            total: 100,
            current_path: "test.bin",
            path_hash: 0,
        };
        assert!((start.percent() - 0.0).abs() < f64::EPSILON);

        // End
        let end = ExtractProgress {
            current: 100,
            total: 100,
            current_path: "test.bin",
            path_hash: 0,
        };
        assert!((end.percent() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_extract_progress_zero_total() {
        let progress = ExtractProgress {
            current: 0,
            total: 0,
            current_path: "test.bin",
            path_hash: 0,
        };

        // Should not panic, returns 0.0
        assert!((progress.percent() - 0.0).abs() < f64::EPSILON);
    }

    // =============================================================================
    // WadExtractor Integration Tests
    // =============================================================================

    #[test]
    fn test_extract_uncompressed_chunk() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with test data
        let test_data = b"Hello, World!";
        let mut source = MockWadSource::new();
        let offset = source.write_at(1000, test_data);

        let chunk = create_uncompressed_chunk(0x1234567890abcdef, offset, test_data);
        let chunks = WadChunks::from_iter([chunk]);

        // Create resolver and extractor
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234567890abcdef, "test/hello.txt".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("test/hello.txt"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);

        // Verify file was created with correct content
        let extracted_path = temp_dir.path().join("test/hello.txt");
        assert!(extracted_path.exists());

        let content = fs::read_to_string(&extracted_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_extract_gzip_chunk() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with gzip-compressed test data
        let test_data = b"This is gzip compressed data!";
        let mut source = MockWadSource::new();
        let (offset, compressed_size) = source.write_gzip_at(1000, test_data);

        let chunk = create_gzip_chunk(0xabcdef1234567890, offset, compressed_size, test_data.len());
        let chunks = WadChunks::from_iter([chunk]);

        // Create resolver and extractor
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0xabcdef1234567890, "compressed/data.txt".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("compressed/data.txt"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);

        // Verify file was created with correct content
        let extracted_path = temp_dir.path().join("compressed/data.txt");
        assert!(extracted_path.exists());

        let content = fs::read_to_string(&extracted_path).unwrap();
        assert_eq!(content, "This is gzip compressed data!");
    }

    #[test]
    fn test_extract_all_chunks() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with multiple chunks
        let mut source = MockWadSource::new();

        let data1 = b"File one content";
        let data2 = b"File two content";
        let data3 = b"File three content";

        let offset1 = source.write_at(1000, data1);
        let offset2 = source.write_at(2000, data2);
        let offset3 = source.write_at(3000, data3);

        let chunk1 = create_uncompressed_chunk(0x1111, offset1, data1);
        let chunk2 = create_uncompressed_chunk(0x2222, offset2, data2);
        let chunk3 = create_uncompressed_chunk(0x3333, offset3, data3);

        let chunks = WadChunks::from_iter([chunk1, chunk2, chunk3]);

        // Create resolver
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "dir1/file1.txt".to_string());
        resolver.insert(0x2222, "dir2/file2.txt".to_string());
        resolver.insert(0x3333, "dir3/file3.txt".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let report = extractor.extract_all(&mut wad, output_path).unwrap();

        assert_eq!(report.extracted, 3);

        // Verify all files were created
        assert!(temp_dir.path().join("dir1/file1.txt").exists());
        assert!(temp_dir.path().join("dir2/file2.txt").exists());
        assert!(temp_dir.path().join("dir3/file3.txt").exists());
    }

    #[test]
    fn test_extract_with_path_filter() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with multiple chunks
        let mut source = MockWadSource::new();

        let data1 = b"Assets file";
        let data2 = b"Data file";

        let offset1 = source.write_at(1000, data1);
        let offset2 = source.write_at(2000, data2);

        let chunk1 = create_uncompressed_chunk(0x1111, offset1, data1);
        let chunk2 = create_uncompressed_chunk(0x2222, offset2, data2);

        let chunks = WadChunks::from_iter([chunk1, chunk2]);

        // Create resolver
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "assets/file1.txt".to_string());
        resolver.insert(0x2222, "data/file2.txt".to_string());

        // Create extractor with prefix filter
        let filter = PrefixFilter::new("assets/");
        let extractor = WadExtractor::new(&resolver).with_filter(filter);

        let mut wad = source.into_wad(chunks);
        let report = extractor.extract_all(&mut wad, output_path).unwrap();

        // Only assets/ file should be extracted
        assert_eq!(report.extracted, 1);
        assert_eq!(report.skipped_by_filter, 1);
        assert!(temp_dir.path().join("assets/file1.txt").exists());
        assert!(!temp_dir.path().join("data/file2.txt").exists());
    }

    #[test]
    fn test_extract_with_type_filter() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source
        let mut source = MockWadSource::new();

        // PNG magic bytes + some data
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG header
            0x00, 0x00, 0x00, 0x00, // Extra data
        ];
        // Random non-PNG data
        let other_data = b"Random text data";

        let offset1 = source.write_at(1000, &png_data);
        let offset2 = source.write_at(2000, other_data);

        let chunk1 = create_uncompressed_chunk(0x1111, offset1, &png_data);
        let chunk2 = create_uncompressed_chunk(0x2222, offset2, other_data);

        let chunks = WadChunks::from_iter([chunk1, chunk2]);

        // Create resolver
        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "images/test.png".to_string());
        resolver.insert(0x2222, "text/readme.txt".to_string());

        // Create extractor with type filter (only PNG)
        let extractor = WadExtractor::new(&resolver).with_type_filter(vec![LeagueFileKind::Png]);

        let mut wad = source.into_wad(chunks);
        let report = extractor.extract_all(&mut wad, output_path).unwrap();

        // Only PNG file should be extracted
        assert_eq!(report.extracted, 1);
        assert_eq!(report.skipped_by_filter, 1);
        assert!(temp_dir.path().join("images/test.png").exists());
        assert!(!temp_dir.path().join("text/readme.txt").exists());
    }

    #[test]
    fn test_extract_progress_callback() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source
        let mut source = MockWadSource::new();
        let data = b"Test data";
        let offset = source.write_at(1000, data);

        let chunk = create_uncompressed_chunk(0x1234, offset, data);
        let chunks = WadChunks::from_iter([chunk]);

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234, "test.txt".to_string());

        // Track progress calls
        let progress_count = Arc::new(AtomicUsize::new(0));
        let progress_count_clone = progress_count.clone();

        let extractor = WadExtractor::new(&resolver).on_progress(move |progress| {
            progress_count_clone.fetch_add(1, Ordering::SeqCst);
            assert_eq!(progress.total, 1);
            assert_eq!(progress.current_path(), "test.txt");
        });

        let mut wad = source.into_wad(chunks);
        extractor.extract_all(&mut wad, output_path).unwrap();

        // Progress callback should have been called once
        assert_eq!(progress_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_extract_hex_path_gets_extension() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with PNG data
        let mut source = MockWadSource::new();
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG header
            0x00, 0x00, 0x00, 0x00,
        ];
        let offset = source.write_at(1000, &png_data);

        let chunk = create_uncompressed_chunk(0x1234567890abcdef, offset, &png_data);
        let chunks = WadChunks::from_iter([chunk]);

        // Use HexPathResolver - no known path, just hex
        let resolver = HexPathResolver;
        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("1234567890abcdef"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);

        // File should have .png extension added based on magic bytes
        assert!(temp_dir.path().join("1234567890abcdef.png").exists());
    }

    #[test]
    fn test_extract_path_without_extension_gets_ltk() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with PNG data (known type)
        let mut source = MockWadSource::new();
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x00,
        ];
        let offset = source.write_at(1000, &png_data);

        let chunk = create_uncompressed_chunk(0x1234, offset, &png_data);
        let chunks = WadChunks::from_iter([chunk]);

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234, "assets/noextension".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("assets/noextension"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);

        // File should have .ltk.png suffix
        assert!(temp_dir.path().join("assets/noextension.ltk.png").exists());
    }

    #[test]
    fn test_extract_path_without_extension_unknown_type() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        // Create mock WAD source with unknown data
        let mut source = MockWadSource::new();
        let unknown_data = b"Unknown file type content";
        let offset = source.write_at(1000, unknown_data);

        let chunk = create_uncompressed_chunk(0x1234, offset, unknown_data);
        let chunks = WadChunks::from_iter([chunk]);

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234, "assets/noextension".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("assets/noextension"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);

        // File should have only .ltk suffix (no type extension)
        assert!(temp_dir.path().join("assets/noextension.ltk").exists());
    }

    #[test]
    fn test_extract_creates_nested_directories() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let mut source = MockWadSource::new();
        let data = b"Deeply nested file";
        let offset = source.write_at(1000, data);

        let chunk = create_uncompressed_chunk(0x1234, offset, data);
        let chunks = WadChunks::from_iter([chunk]);

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1234, "a/b/c/d/e/deep.txt".to_string());

        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let chunk = *wad.chunks().iter().next().unwrap();
        let chunk_data = wad.load_chunk_decompressed(&chunk).unwrap();

        let result = extractor
            .extract_chunk_data(
                &chunk,
                &chunk_data,
                Utf8Path::new("a/b/c/d/e/deep.txt"),
                output_path,
            )
            .unwrap();

        assert_eq!(result, ExtractResult::Extracted);
        assert!(temp_dir.path().join("a/b/c/d/e/deep.txt").exists());
    }

    #[test]
    fn test_extract_empty_chunks_returns_zero() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let source = MockWadSource::new();
        let chunks = WadChunks::from_iter([]);

        let resolver = HexPathResolver;
        let extractor = WadExtractor::new(&resolver);

        let mut wad = source.into_wad(chunks);
        let report = extractor.extract_all(&mut wad, output_path).unwrap();

        assert_eq!(report, ExtractReport::default());
    }

    #[test]
    fn test_extractor_builder_pattern() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let resolver = HexPathResolver;
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        // Test that builder pattern works correctly
        let _extractor = WadExtractor::new(&resolver)
            .with_filter(PrefixFilter::new("assets/"))
            .with_type_filter(vec![LeagueFileKind::Png, LeagueFileKind::Jpeg])
            .on_progress(move |_| {
                called_clone.store(true, Ordering::SeqCst);
            });

        // Builder compiles and type inference works
    }

    // =============================================================================
    // Report, Layout, Policy and Selection Tests
    // =============================================================================

    const PNG_MAGIC: [u8; 12] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 0];

    /// Three uncompressed text chunks, each under a directory of its own.
    fn three_file_wad() -> (Wad<MockWadSource>, HashMapPathResolver) {
        let mut source = MockWadSource::new();
        let offset1 = source.write_at(1000, b"File one content");
        let offset2 = source.write_at(2000, b"File two content");
        let offset3 = source.write_at(3000, b"File three content");

        let chunks = WadChunks::from_iter([
            create_uncompressed_chunk(0x1111, offset1, b"File one content"),
            create_uncompressed_chunk(0x2222, offset2, b"File two content"),
            create_uncompressed_chunk(0x3333, offset3, b"File three content"),
        ]);

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "dir1/file1.txt".to_string());
        resolver.insert(0x2222, "dir2/file2.txt".to_string());
        resolver.insert(0x3333, "dir3/file3.txt".to_string());

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

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "images/test.png".to_string());
        resolver.insert(0x2222, "text/readme.txt".to_string());

        let mut wad = source.into_wad(chunks);
        let report = WadExtractor::new(&resolver)
            .extract_all(&mut wad, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2);
        assert_eq!(report.skipped_existing, 0);
        assert_eq!(report.skipped_by_filter, 0);
        assert_eq!(report.bytes_written, (PNG_MAGIC.len() + text.len()) as u64);
        assert_eq!(report.by_kind.get(&LeagueFileKind::Png), Some(&1));
        assert_eq!(report.by_kind.values().sum::<usize>(), 2);
        assert!(!report.cancelled);
    }

    #[test]
    fn extract_chunks_takes_a_subset() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();
        let (mut wad, resolver) = three_file_wad();

        let wanted = [
            *wad.chunks().get(0x1111).unwrap(),
            *wad.chunks().get(0x3333).unwrap(),
        ];
        let report = WadExtractor::new(&resolver)
            .extract_chunks(&mut wad, &wanted, output_path)
            .unwrap();

        assert_eq!(report.extracted, 2);
        assert!(temp_dir.path().join("dir1/file1.txt").exists());
        assert!(!temp_dir.path().join("dir2/file2.txt").exists());
        assert!(temp_dir.path().join("dir3/file3.txt").exists());
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

        let mut resolver = HashMapPathResolver::default();
        resolver.insert(0x1111, "a/same.txt".to_string());
        resolver.insert(0x2222, "b/same.txt".to_string());

        // One worker, so the chunks land in the order they are read.
        let mut wad = source.into_wad(chunks);
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
    fn extract_chunk_data_honors_the_skip_policy() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let output_path = Utf8Path::from_path(temp_dir.path()).unwrap();

        let existing = temp_dir.path().join("test/hello.txt");
        fs::create_dir_all(existing.parent().unwrap()).unwrap();
        fs::write(&existing, "kept").unwrap();

        let mut source = MockWadSource::new();
        let data = b"Hello, World!";
        let offset = source.write_at(1000, data);
        let chunk = create_uncompressed_chunk(0x1234, offset, data);

        let resolver = HexPathResolver;
        let extractor =
            WadExtractor::new(&resolver).with_existing_file_policy(ExistingFilePolicy::Skip);

        let result = extractor
            .extract_chunk_data(&chunk, data, Utf8Path::new("test/hello.txt"), output_path)
            .unwrap();

        assert_eq!(result, ExtractResult::SkippedExisting);
        assert_eq!(fs::read_to_string(&existing).unwrap(), "kept");
    }

    // =============================================================================
    // Regex Filter Tests (feature-gated)
    // =============================================================================

    #[cfg(feature = "regex")]
    mod regex_tests {
        use super::*;

        #[test]
        fn test_regex_filter_matches_pattern() {
            let filter = RegexFilter::new(r"^assets/.*\.bin$").unwrap();

            assert!(filter.matches("assets/champions/aatrox.bin"));
            assert!(filter.matches("assets/test.bin"));
            assert!(!filter.matches("data/test.bin"));
            assert!(!filter.matches("assets/test.png"));
        }

        #[test]
        fn test_regex_filter_complex_patterns() {
            let filter = RegexFilter::new(r"champions/(aatrox|ahri|akali)/").unwrap();

            assert!(filter.matches("assets/champions/aatrox/skin0.bin"));
            assert!(filter.matches("data/champions/ahri/animations.anm"));
            assert!(filter.matches("champions/akali/test"));
            assert!(!filter.matches("champions/ashe/test"));
        }

        #[test]
        fn test_regex_filter_invalid_pattern_returns_none() {
            let filter = RegexFilter::new(r"[invalid");
            assert!(filter.is_none());
        }

        #[test]
        fn test_regex_filter_from_compiled_regex() {
            let regex = regex::Regex::new(r"\.png$").unwrap();
            let filter = RegexFilter::from_regex(regex);

            assert!(filter.matches("test.png"));
            assert!(!filter.matches("test.jpg"));
        }
    }
}
