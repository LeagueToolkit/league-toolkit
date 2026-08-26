//! WAD chunk extraction utilities.
//!
//! This module provides abstractions for extracting chunks from WAD archives to disk.
//!
//! # Example
//!
//! ```no_run
//! use std::collections::HashMap;
//! use std::fs::File;
//! use ltk_wad::{Wad, WadExtractor, WadHash};
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//!
//! // Any `HashMap<WadHash, String>` is a resolver. Load a hash table into one.
//! let names: HashMap<WadHash, String> = HashMap::new();
//!
//! let mut extractor = WadExtractor::new(&names).on_progress(|progress| {
//!     println!("{:.1}% {}", progress.fraction() * 100.0, progress.path());
//! });
//!
//! let report = extractor.extract_all(&mut wad, "/output/path")?;
//! println!("{report}");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Extracting a selection
//!
//! [`WadExtractor::extract_chunks`] takes path hashes, so a caller that knows
//! which chunks it wants extracts those alone:
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_wad::{Wad, WadExtractor, WadHash, NoResolver, ExtractLayout, ExistingFilePolicy};
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//!
//! let wanted = [WadHash(0x1234567890abcdef), WadHash(0xfedcba0987654321)];
//! let mut extractor = WadExtractor::new(&NoResolver)
//!     .with_layout(ExtractLayout::Flat)
//!     .with_existing_file_policy(ExistingFilePolicy::Skip);
//! let report = extractor.extract_chunks(&mut wad, wanted, "/output/path")?;
//! println!("{} written, {} were there already", report.extracted, report.skipped_existing);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # How a chunk is named on disk
//!
//! - A path the resolver knows stays as it is.
//! - A chunk the resolver has no name for lands under its hash as sixteen hex
//!   digits. The extension is the one its bytes identify as, when they identify
//!   as anything.
//! - A name a directory holds takes a `.ltk` suffix: `foo.bin` becomes
//!   `foo.bin.ltk`, because a file cannot share a name with a directory. The
//!   suffix is added and never substituted, so stripping a trailing `.ltk`
//!   gives the name back, hex name as much as path. Nothing else moves a chunk
//!   off the name its path gave it, which is what a caller hashing an extracted
//!   file's path back to its chunk needs.
//! - A WAD can name both `x` and `x/y`; no file system holds both. `x` is the
//!   one that moves, to `x.ltk`, so both paths come through the extraction.
//!   Which of the two moves is worked out over the extraction's own paths
//!   before it writes any of them, so one archive and one hash table give one
//!   output tree every run, whatever order the chunks are written in.
//! - A name the file system refuses outright, most often the Windows
//!   long-path limit, becomes `<hash>.<ext>` in the output directory itself,
//!   losing the directories the path named.
//!
//! An extraction never ends over a pair of its own paths that cannot both
//! stand. It moves one of them and lists it under [`ExtractReport::displaced`].
//!
//! [`NamingPolicy::Lossless`] changes two of those. A nameless chunk keeps the
//! bare hash, inventing no extension, and a chunk whose path another chunk
//! claimed first takes the `.ltk` suffix rather than go unwritten. Every chunk
//! then lands, and every name reads back as the path the resolver gave. Only a
//! name the file system refuses outright still falls back to the hash, which no
//! suffix can mend.
//!
//! # Paths the extraction will not write
//!
//! A resolver's paths are untrusted: a hash table is a third-party download,
//! and name recovery reads paths out of the archive itself. Two kinds never
//! reach the file system, and [`ExtractReport::displaced`] lists both:
//!
//! - A path that would name a file the caller did not ask for: one leaving the
//!   output directory, or one a host would quietly read as something else.
//!   [`DisplacedChunk::path`] carries the rejected path.
//! - A path a chunk of the same extraction claimed already. The first file
//!   stays; the second chunk is not written over it. Two hashes resolving to
//!   one path means the resolver is wrong about one of them, and an extraction
//!   that overwrote in silence would lose the difference.
//!
//! The report lists a chunk the file system refused the name of as well, so
//! that rename is not silent either.
//!
//! A chunk no resolver names can still get its name from the archive itself.
//! [`WadExtractor::with_name_recovery`] reads the `.bin` files for it first.
//! Read [`NameRecovery`] for how.
//!
//! # Parallelism
//!
//! [`extract_all`](WadExtractor::extract_all) and
//! [`extract_chunks`](WadExtractor::extract_chunks) read the archive on the
//! calling thread, in the order its chunks are laid out so the whole read is
//! one forward sweep, and hand each chunk to a worker that decompresses and
//! writes it. The worker count bounds the channel between the two, so memory
//! holds a few chunks whatever the archive holds. The resolver, the path
//! filter and the progress callback run on the calling thread only, so none of
//! them needs to be [`Sync`]. The progress callback hears of each chunk once
//! the chunk is done.

mod naming;
mod report;
mod resolver;
mod writer;

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    fmt,
    io::{Read, Seek},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Mutex, MutexGuard, PoisonError,
    },
    thread,
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;
use ltk_hash::WadHash;

use crate::{ChunkDecoder, NameRecovery, RecoveredNames, Wad, WadChunk, WadError};

pub use self::{
    naming::{chunk_hash_of, strip_ltk_suffix, NamingPolicy},
    report::{DisplacedChunk, ExtractProgress, ExtractReport, ExtractResult, PathIssue},
    resolver::{hex_chunk_hash, hex_name, is_hex_chunk_path, NoResolver, PathResolver},
    writer::{ExistingFilePolicy, ExtractLayout},
};

use self::{
    naming::{is_evil, DirectoryPaths},
    report::ChunkOutcome,
    writer::ChunkWriter,
};

pub(crate) use self::resolver::resolve_all_checked;

/// Most workers an extraction starts unless [`WadExtractor::with_workers`] says
/// otherwise. Each worker holds a compressed and a decompressed chunk at once,
/// and a wide machine would otherwise hold dozens of the largest ones.
const DEFAULT_WORKER_CAP: usize = 8;

type PathFilter<'a> = Box<dyn Fn(&str) -> bool + 'a>;
type ProgressCallback<'a> = Box<dyn FnMut(&ExtractProgress<'_>) + 'a>;

/// Configuration and execution of WAD chunk extraction.
///
/// Build one with [`new`](Self::new), configure it with the `with_*` methods,
/// and run it with [`extract_all`](Self::extract_all) or
/// [`extract_chunks`](Self::extract_chunks). One extractor runs any number of
/// extractions.
pub struct WadExtractor<'a> {
    resolver: &'a dyn PathResolver,
    filter: Option<PathFilter<'a>>,
    type_filter: Option<Vec<LeagueFileKind>>,
    progress: Option<ProgressCallback<'a>>,
    layout: ExtractLayout,
    existing: ExistingFilePolicy,
    naming: NamingPolicy,
    cancel: Option<&'a AtomicBool>,
    workers: Option<NonZeroUsize>,
    recover_names: bool,
}

impl fmt::Debug for WadExtractor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WadExtractor")
            .field("layout", &self.layout)
            .field("existing", &self.existing)
            .field("naming", &self.naming)
            .field("type_filter", &self.type_filter)
            .field("has_filter", &self.filter.is_some())
            .field("has_progress", &self.progress.is_some())
            .field("workers", &self.workers)
            .field("recover_names", &self.recover_names)
            .finish_non_exhaustive()
    }
}

impl<'a> WadExtractor<'a> {
    /// An extractor that names chunks through `resolver`.
    pub fn new(resolver: &'a dyn PathResolver) -> Self {
        Self {
            resolver,
            filter: None,
            type_filter: None,
            progress: None,
            layout: ExtractLayout::default(),
            existing: ExistingFilePolicy::default(),
            naming: NamingPolicy::default(),
            cancel: None,
            workers: None,
            recover_names: false,
        }
    }

    /// Extract only the chunks whose path `filter` accepts.
    ///
    /// The filter sees the path the resolver gave, or the hash as sixteen hex
    /// digits when it gave none. It runs on the calling thread.
    ///
    /// An extraction asks the filter about each chunk once, before it writes
    /// any of them, because which paths it will write decides which of them a
    /// directory has to hold.
    pub fn with_filter(mut self, filter: impl Fn(&str) -> bool + 'a) -> Self {
        self.filter = Some(Box::new(filter));
        self
    }

    /// Extract only the chunks whose bytes identify as one of `kinds`.
    ///
    /// A chunk's kind is not known until its bytes are decompressed, so this
    /// filter cannot say up front which paths the extraction will write. A
    /// chunk it drops still counts as a directory of the path it names, so a
    /// path that a dropped chunk made a directory of still takes its `.ltk`
    /// suffix. The extraction reports the move under
    /// [`ExtractReport::displaced`], and the path filter, which does run up
    /// front, has no such cost.
    pub fn with_type_filter(mut self, kinds: impl IntoIterator<Item = LeagueFileKind>) -> Self {
        self.type_filter = Some(kinds.into_iter().collect());
        self
    }

    /// Hear of each chunk once it is done, skipped chunks included.
    ///
    /// The callback runs on the calling thread.
    pub fn on_progress(mut self, callback: impl FnMut(&ExtractProgress<'_>) + 'a) -> Self {
        self.progress = Some(Box::new(callback));
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

    /// Set whether names can be read back as the paths they came from.
    ///
    /// The default is [`NamingPolicy::Descriptive`].
    pub fn with_naming_policy(mut self, policy: NamingPolicy) -> Self {
        self.naming = policy;
        self
    }

    /// Stop early once `flag` reads `true`.
    ///
    /// The reader checks the flag before it reads each chunk from the archive.
    /// Chunks already handed to a worker still land, and
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
    /// cannot name, before the extraction writes anything.
    ///
    /// Read [`NameRecovery`] for what it costs and what it finds. The names
    /// land in [`ExtractReport::recovered`], and the extraction uses the same
    /// workers and cancel flag for the read.
    pub fn with_name_recovery(mut self) -> Self {
        self.recover_names = true;
        self
    }

    /// Extract every chunk of `wad` into `output_dir`.
    ///
    /// # Errors
    ///
    /// Fails on the first chunk the extractor cannot read, decompress or write,
    /// with a [`WadError::Chunk`] that names it. Chunks written before it stay
    /// on disk.
    pub fn extract_all<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        output_dir: impl AsRef<Utf8Path>,
    ) -> Result<ExtractReport, WadError> {
        let chunks: Vec<WadChunk> = wad.chunks().iter().copied().collect();
        self.run(wad, chunks, Vec::new(), output_dir.as_ref())
    }

    /// Extract the chunks of `wad` with the given path hashes into
    /// `output_dir`.
    ///
    /// A hash given twice counts once. A hash the archive holds no chunk for
    /// lands under [`ExtractReport::missing`] and is not an error. The chunks
    /// are read in the order the archive lays them out.
    ///
    /// # Errors
    ///
    /// Fails on the first chunk the extractor cannot read, decompress or write,
    /// with a [`WadError::Chunk`] that names it. Chunks written before it stay
    /// on disk.
    pub fn extract_chunks<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        path_hashes: impl IntoIterator<Item = WadHash>,
        output_dir: impl AsRef<Utf8Path>,
    ) -> Result<ExtractReport, WadError> {
        let mut seen = HashSet::new();
        let mut chunks = Vec::new();
        let mut missing = Vec::new();
        for path_hash in path_hashes {
            if !seen.insert(path_hash) {
                continue;
            }
            match wad.chunks().get(path_hash) {
                Some(chunk) => chunks.push(*chunk),
                None => missing.push(path_hash),
            }
        }
        self.run(wad, chunks, missing, output_dir.as_ref())
    }

    /// Name every chunk, and settle what is decided before any write: the
    /// paths the extraction refuses, and the ones the path filter drops.
    ///
    /// Every chunk is resolved here and nowhere else. The rename needs all of
    /// the extraction's paths before it can say which of them a directory has
    /// to hold, so the paths are read once, up front, and carried from here to
    /// the worker that writes each chunk.
    ///
    /// The whole archive goes to the resolver in one
    /// [`resolve_all`](PathResolver::resolve_all), which is what lets a
    /// resolver reading a compressed store make one pass over it.
    fn resolve_chunks(&self, chunks: Vec<WadChunk>, resolver: &dyn PathResolver) -> Vec<Named> {
        let path_hashes: Vec<WadHash> = chunks.iter().map(|chunk| chunk.path_hash).collect();
        let resolved = resolve_all_checked(resolver, &path_hashes);

        chunks
            .into_iter()
            .zip(resolved)
            .map(|(chunk, resolved)| {
                let (path, named) = match resolved {
                    Some(path) => (path, true),
                    None => (hex_name(chunk.path_hash), false),
                };
                /* Refused before filtered, so a caller's selection cannot mask
                the fact that its resolver handed out a path the extraction
                will not write. */
                let refused = if is_evil(&path) {
                    Some(Refusal::Rejected)
                } else if self
                    .filter
                    .as_ref()
                    .is_some_and(|filter| !filter(path.as_str()))
                {
                    Some(Refusal::Filtered)
                } else {
                    None
                };
                Named {
                    chunk,
                    path,
                    named,
                    refused,
                }
            })
            .collect()
    }

    fn run<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        mut chunks: Vec<WadChunk>,
        missing: Vec<WadHash>,
        output_dir: &Utf8Path,
    ) -> Result<ExtractReport, WadError> {
        let workers = self.workers.map_or_else(default_workers, NonZeroUsize::get);

        /* The archive keeps its chunks in hash order; sorted by offset, the
        reader's whole pass is one forward sweep. */
        chunks.sort_unstable_by_key(|chunk| chunk.data_offset);

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

        let total = chunks.len();
        let chunks = self.resolve_chunks(chunks, &resolver);

        let directories = match self.layout {
            ExtractLayout::Paths => directory_paths(&chunks),
            /* A flat layout writes file names into the output directory itself
            and makes no directory, so no path of it can be one. */
            ExtractLayout::Flat => DirectoryPaths::default(),
        };

        let shared = Shared {
            writer: ChunkWriter {
                layout: self.layout,
                existing: self.existing,
                naming: self.naming,
                type_filter: self.type_filter.as_deref(),
                output_dir,
                directories,
                claimed: Mutex::default(),
            },
            report: Mutex::default(),
            failure: Mutex::default(),
        };
        let (sender, receiver) = mpsc::sync_channel::<Job>(workers);
        let receiver = Mutex::new(receiver);
        let (done_sender, done) = mpsc::channel::<Done>();

        let mut reader = Reader {
            cancel: self.cancel,
            progress: Progress {
                callback: self.progress.as_deref_mut(),
                done: 0,
                total,
            },
        };

        let cancelled = thread::scope(|scope| {
            let shared = &shared;
            let receiver = &receiver;
            for _ in 0..workers {
                let done_sender = done_sender.clone();
                scope.spawn(move || shared.run_worker(receiver, &done_sender));
            }
            /* The workers hold the clones. The drain below ends once every
            sender is gone, and this one would never go on its own. */
            drop(done_sender);

            let result = reader.read_chunks(wad, chunks, &sender, shared, &done);
            /* A worker exits, and lets go of its done sender, only once every
            job sender is gone. */
            drop(sender);
            for finished in done.iter() {
                reader.progress.report(&finished);
            }
            result
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
        report.missing = missing;
        report.recovered = recovered;
        Ok(report)
    }
}

pub(crate) fn default_workers() -> usize {
    thread::available_parallelism().map_or(1, |count| count.get().min(DEFAULT_WORKER_CAP))
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// One chunk of an extraction under the name it will be written under.
///
/// Built once, before anything is written, because the rename cannot be
/// settled chunk by chunk: whether a path has to move depends on every other
/// path of the same extraction.
struct Named {
    chunk: WadChunk,
    /// The resolved path, or the hash as sixteen hex digits when no resolver
    /// named the chunk.
    path: String,
    /// Whether a resolver named the chunk.
    named: bool,
    /// What stopped the chunk before any of it was written, when something did.
    refused: Option<Refusal>,
}

/// Why a chunk never reached a worker.
#[derive(Debug, Clone, Copy)]
enum Refusal {
    /// Its path is one the extraction will not write.
    Rejected,
    /// The path filter did not accept it.
    Filtered,
}

/// The directories the chunks of one extraction name between them.
///
/// A chunk that will not be written makes no directory, and neither does one
/// under a bare hash, since a hash names no directory. The type filter cannot
/// be applied here, because a chunk's kind is not known until its bytes are
/// decompressed, so a chunk the type filter goes on to drop still counts as a
/// directory of the path it names.
fn directory_paths(chunks: &[Named]) -> DirectoryPaths {
    DirectoryPaths::of(chunks.iter().filter_map(|resolved| {
        (resolved.named && resolved.refused.is_none()).then_some(resolved.path.as_str())
    }))
}

/// One chunk on its way from the reader to a worker.
struct Job {
    chunk: WadChunk,
    path: String,
    named: bool,
    raw: Box<[u8]>,
}

/// One chunk on its way back from a worker, for the progress callback.
struct Done {
    path_hash: WadHash,
    path: String,
    named: bool,
    output_path: Option<Utf8PathBuf>,
    outcome: ChunkOutcome,
}

impl Done {
    /// The report entry for this chunk, when something was wrong with its path.
    fn displaced(&self, issue: Option<PathIssue>) -> Option<DisplacedChunk> {
        Some(DisplacedChunk {
            path_hash: self.path_hash,
            path: self.path.clone(),
            issue: issue?,
        })
    }
}

/// The progress callback and its count. Both stay on the reader thread.
struct Progress<'c, 'a> {
    callback: Option<&'c mut (dyn FnMut(&ExtractProgress<'_>) + 'a)>,
    done: usize,
    total: usize,
}

impl Progress<'_, '_> {
    fn report(&mut self, finished: &Done) {
        self.done += 1;
        if let Some(callback) = &mut self.callback {
            callback(&ExtractProgress {
                done: self.done,
                total: self.total,
                path_hash: finished.path_hash,
                path: &finished.path,
                named: finished.named,
                output_path: finished.output_path.as_deref(),
                result: finished.outcome.into(),
                bytes: finished.outcome.bytes(),
            });
        }
    }
}

/// The half of the extractor that runs on the calling thread.
struct Reader<'r, 'a> {
    cancel: Option<&'r AtomicBool>,
    progress: Progress<'r, 'a>,
}

impl Reader<'_, '_> {
    /// Feed the workers, chunk by chunk, and say whether the cancel flag stopped it.
    fn read_chunks<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        chunks: Vec<Named>,
        sender: &mpsc::SyncSender<Job>,
        shared: &Shared<'_>,
        done: &mpsc::Receiver<Done>,
    ) -> Result<bool, WadError> {
        for Named {
            chunk,
            path,
            named,
            refused,
        } in chunks
        {
            if self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Ok(true);
            }
            if shared.failed() {
                return Ok(false);
            }

            if let Some(refusal) = refused {
                let (outcome, issue) = match refusal {
                    Refusal::Rejected => {
                        (ChunkOutcome::SkippedRejectedPath, Some(PathIssue::Rejected))
                    }
                    Refusal::Filtered => (ChunkOutcome::SkippedByPath, None),
                };
                let finished = Done {
                    path_hash: chunk.path_hash,
                    path,
                    named,
                    output_path: None,
                    outcome,
                };
                lock(&shared.report).record_chunk(finished.outcome, finished.displaced(issue));
                self.progress.report(&finished);
                continue;
            }

            let raw = wad
                .load_chunk_raw(&chunk)
                .map_err(|error| WadError::chunk(chunk.path_hash, &path, error))?;
            let job = Job {
                chunk,
                path,
                named,
                raw,
            };
            /* Refused only once every worker is gone, which takes a panic. */
            if sender.send(job).is_err() {
                break;
            }
            for finished in done.try_iter() {
                self.progress.report(&finished);
            }
        }
        Ok(false)
    }
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
    /// After a failure the worker drains and drops the jobs rather than writes
    /// them, so a reader blocked on a full channel sees the failure too.
    fn run_worker(&self, receiver: &Mutex<mpsc::Receiver<Job>>, done: &mpsc::Sender<Done>) {
        let mut decoder = ChunkDecoder::new();
        loop {
            let Ok(job) = lock(receiver).recv() else {
                return;
            };
            if self.failed() {
                continue;
            }
            match self.writer.write(&job, &mut decoder) {
                Ok(written) => {
                    /* `path` moves out of `job`, so `named` is read first. */
                    let finished = Done {
                        path_hash: job.chunk.path_hash,
                        named: job.named,
                        path: job.path,
                        output_path: written.path,
                        outcome: written.outcome,
                    };
                    lock(&self.report)
                        .record_chunk(finished.outcome, finished.displaced(written.issue));
                    /* The receiver outlives every worker, so this cannot fail. */
                    let _ = done.send(finished);
                }
                Err(error) => {
                    let mut failure = lock(&self.failure);
                    if failure.is_none() {
                        *failure = Some(WadError::chunk(job.chunk.path_hash, &job.path, error));
                    }
                }
            }
        }
    }
}
