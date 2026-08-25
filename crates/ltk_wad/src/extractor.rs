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
//! - A name the file system refuses as too long becomes `<hash>.<ext>` in the
//!   output directory itself, losing the directories the path named.
//!
//! An extraction never ends over a pair of its own paths that cannot both
//! stand. It moves one of them and lists it under [`ExtractReport::displaced`].
//!
//! # Paths the extraction will not write
//!
//! A resolver's paths are untrusted: a hash table is a third-party download,
//! and name recovery reads paths out of the archive itself. Two kinds never
//! reach the file system, and [`ExtractReport::displaced`] lists both:
//!
//! - A path that would name a file the caller did not ask for: one leaving the
//!   output directory, or one a host would quietly read as something else.
//!   [`DisplacedChunk::path`] carries the path that was refused.
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
//! [`extract_chunks`](WadExtractor::extract_chunks) read the archive in order on
//! the calling thread and hand each chunk to a worker that decompresses and
//! writes it. The worker count bounds the channel between the two, so memory
//! holds a few chunks whatever the archive holds. The resolver, the path
//! filter and the progress callback run on the calling thread only, so none of
//! them needs to be [`Sync`]. The progress callback hears of each chunk once
//! the chunk is done.

#[cfg(test)]
mod tests;

use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap, HashSet},
    fmt,
    fs::{self, OpenOptions},
    hash::BuildHasher,
    io::{self, Read, Seek, Write as _},
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex, MutexGuard, PoisonError,
    },
    thread,
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;
use ltk_hash::WadHash;

use crate::{ChunkDecoder, NameRecovery, RecoveredNames, Wad, WadChunk, WadError};

/// Names chunks by their path hash.
///
/// A WAD stores the hash of each chunk's path and not the path. A resolver
/// supplies the path, from a hash table or any other source, and answers
/// `None` for a hash it has no name for. The extractor writes such a chunk
/// under its hash as sixteen hex digits.
///
/// Every `HashMap<WadHash, String>` is a resolver, and so is a reference, a `Box`
/// or an `Arc` of any resolver.
pub trait PathResolver {
    /// The path of `path_hash`, or `None` when the resolver has no name for it.
    fn resolve(&self, path_hash: WadHash) -> Option<String>;

    /// Whether the resolver names `path_hash`.
    ///
    /// The default calls [`resolve`](Self::resolve). A resolver that can
    /// answer without building the string should override it.
    fn is_known(&self, path_hash: WadHash) -> bool {
        self.resolve(path_hash).is_some()
    }
}

impl<R: PathResolver + ?Sized> PathResolver for &R {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }
}

impl<R: PathResolver + ?Sized> PathResolver for Box<R> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }
}

impl<R: PathResolver + ?Sized> PathResolver for Arc<R> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }
}

impl<S: BuildHasher> PathResolver for HashMap<WadHash, String, S> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        self.get(&path_hash).cloned()
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        self.contains_key(&path_hash)
    }
}

/// A resolver that names nothing, so every chunk lands under its hash.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoResolver;

impl PathResolver for NoResolver {
    fn resolve(&self, _path_hash: WadHash) -> Option<String> {
        None
    }

    fn is_known(&self, _path_hash: WadHash) -> bool {
        false
    }
}

/// A path hash as the sixteen hex digits a nameless chunk lands under.
pub(crate) fn hex_name(path_hash: WadHash) -> String {
    format!("{path_hash:016x}")
}

/// Whether `path` is one an extraction must refuse to write.
///
/// A resolver's paths are untrusted: a hash table is a third-party download,
/// and [`WadExtractor::with_name_recovery`] reads paths out of the archive
/// itself. A path is evil when joining it onto the output directory would not
/// give a plain file plainly under that directory:
///
/// - it starts at a root, a drive or a network share, so the join ignores the
///   output directory;
/// - a component is `..`, which reaches the directory above;
/// - a component holds a `:`, naming a Windows drive or an alternate data
///   stream instead of a file the directory lists;
/// - a component ends in a dot or a space, which Windows strips before it
///   looks the name up, so `notes.txt.` and `notes.txt` are one file under two
///   names and would walk past the check for two chunks claiming one path;
/// - or it names no file at all, holding nothing but separators and `.`.
///
/// The last two rules are Windows behaviour, applied wherever the extraction
/// runs. That is deliberate: one archive and one hash table then give one
/// output tree on every host, and a test on any of them catches a table that
/// would misbehave on Windows. It costs two conditions.
///
/// `path` is read as the raw string a resolver gave. Turning it into a
/// [`Utf8Path`] first would normalise away the very things this looks for, and
/// `/` and `\` both separate components whatever the host, so a table written
/// on Windows cannot escape on Linux or the other way round.
///
/// The check is lexical. It says the joined path cannot name a file outside the
/// output directory; it says nothing about a symlink an output tree already
/// holds.
fn is_evil(path: &str) -> bool {
    if path.starts_with(['/', '\\']) {
        return true;
    }

    let mut names_a_file = false;
    for component in path.split(['/', '\\']) {
        /* `a//b` and a trailing separator each give an empty component, and `.`
        is the directory the walk already stands in. `join` steps over both. */
        match component {
            "" | "." => continue,
            ".." => return true,
            _ => {}
        }
        if component.contains(':') || component.ends_with(['.', ' ']) {
            return true;
        }
        names_a_file = true;
    }

    !names_a_file
}

/// The directories a set of paths names, each a path in its own right.
///
/// A WAD is a flat map from path to bytes, so it can hold both `x` and `x/y`.
/// A file system holds one or the other, so one of the two has to move. Which
/// one is settled against this, built over the paths an extraction resolved
/// before it writes any of them. Settling it at the write instead would follow
/// whichever worker reached the file system first, and two runs of one archive
/// could give two trees.
#[derive(Debug, Default)]
struct DirectoryPaths(HashSet<String>);

impl DirectoryPaths {
    /// The directories `paths` names.
    fn of(paths: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        let mut directories = HashSet::new();
        for path in paths {
            let path = plain_path(path.as_ref());
            for (end, _) in path.match_indices('/') {
                let directory = &path[..end];
                /* The paths of one tree share their directories, so most of
                these are there already and cost nothing. */
                if !directories.contains(directory) {
                    directories.insert(directory.to_owned());
                }
            }
        }
        Self(directories)
    }

    /// Whether a path of the same set needs `path` to be a directory.
    fn holds(&self, path: &str) -> bool {
        self.0.contains(plain_path(path).as_ref())
    }
}

/// `path` with one `/` between its components and nothing else.
///
/// The components are the ones a join steps onto, so an empty one from `a//b`
/// or from a trailing separator is dropped, as is a `.`, and `\` separates as
/// `/` does whatever the host. Borrowed when `path` is in that form already,
/// which nearly every path a hash table holds is.
fn plain_path(path: &str) -> Cow<'_, str> {
    if !path.contains('\\') && !path.split('/').any(|part| matches!(part, "" | ".")) {
        return Cow::Borrowed(path);
    }
    Cow::Owned(
        path.split(['/', '\\'])
            .filter(|part| !matches!(*part, "" | "."))
            .collect::<Vec<_>>()
            .join("/"),
    )
}

/// One chunk done, as [`WadExtractor::on_progress`] reports it.
#[derive(Debug, Clone)]
pub struct ExtractProgress<'a> {
    done: usize,
    total: usize,
    path_hash: WadHash,
    path: &'a str,
    named: bool,
    output_path: Option<&'a Utf8Path>,
    result: ExtractResult,
    bytes: u64,
}

impl ExtractProgress<'_> {
    /// Chunks done so far, this one included.
    pub fn done(&self) -> usize {
        self.done
    }

    /// Chunks the extraction covers, done or not.
    pub fn total(&self) -> usize {
        self.total
    }

    /// [`done`](Self::done) over [`total`](Self::total), from 0.0 to 1.0.
    ///
    /// Zero when there was nothing to do.
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f64 / self.total as f64
        }
    }

    /// The chunk's path hash.
    pub fn path_hash(&self) -> WadHash {
        self.path_hash
    }

    /// The chunk's path, or its hash as sixteen hex digits when nothing named it.
    ///
    /// [`is_named`](Self::is_named) tells the two apart.
    pub fn path(&self) -> &str {
        self.path
    }

    /// Whether a resolver named the chunk.
    ///
    /// `false` means nothing knew the hash, so [`path`](Self::path) is that
    /// hash as sixteen hex digits and not a path the archive was built from.
    ///
    /// A caller sorting an extraction into named and unnamed chunks reads this
    /// rather than [`is_hex_chunk_path`], which cannot tell a real name of
    /// sixteen hex digits from a hash.
    pub fn is_named(&self) -> bool {
        self.named
    }

    /// What became of the chunk.
    pub fn result(&self) -> ExtractResult {
        self.result
    }

    /// Bytes written for the chunk. Zero unless it was extracted.
    pub fn bytes(&self) -> u64 {
        self.bytes
    }

    /// The chunk's file, relative to the extraction's output directory.
    ///
    /// `None` whenever nothing was written: a filter left the chunk out, its
    /// path was one the extraction refuses, or another chunk claimed that path
    /// first. [`result`](Self::result) says which.
    ///
    /// The layout, the `.ltk` suffix and a name the file system refuses can
    /// each make this differ from [`path`](Self::path), so a caller that
    /// indexes what an extraction wrote reads this and not that.
    pub fn output_path(&self) -> Option<&Utf8Path> {
        self.output_path
    }
}

/// What became of one chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExtractResult {
    /// Written to disk.
    Extracted,
    /// Left out by the type filter.
    SkippedByType,
    /// Left out by the path filter.
    SkippedByPath,
    /// Its file existed already, and the policy was [`ExistingFilePolicy::Skip`].
    SkippedExisting,
    /// Its resolved path was one the extraction will not write, so nothing was.
    ///
    /// [`ExtractReport::displaced`] names it and says why.
    SkippedUnwritablePath,
    /// Another chunk of the extraction claimed its path first, so nothing was
    /// written.
    ///
    /// [`ExtractReport::displaced`] names it.
    SkippedDuplicatePath,
}

/// Why a chunk did not land at the path its resolver gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PathIssue {
    /// The path was one the extraction will not write, so nothing was written.
    ///
    /// A resolver's paths are untrusted: a hash table is a third-party
    /// download, and [`with_name_recovery`](WadExtractor::with_name_recovery)
    /// reads paths out of the archive itself, and [`DisplacedChunk::path`]
    /// carries the one that was refused.
    Unwritable,
    /// Another chunk claimed the path first, so nothing was written.
    ///
    /// Two path hashes resolving to one path means the resolver is wrong about
    /// one of them. The extraction keeps the file the first chunk wrote rather
    /// than let the second overwrite it unseen. Which chunk is first follows
    /// write order.
    Duplicate,
    /// The path could not name a file, so the chunk took another name.
    ///
    /// A directory holds the name, so the chunk took a `.ltk` suffix — a WAD
    /// can hold both `x` and `x/y`, which no file system can — or the file
    /// system refused the name outright, the long-path case on Windows most
    /// often, so the chunk took its hash. It is still written, and
    /// [`DisplacedChunk::output_path`] says where.
    Refused,
}

/// One chunk that did not land at the path its resolver gave.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DisplacedChunk {
    /// The chunk's path hash.
    pub path_hash: WadHash,
    /// The path the resolver gave, which the extraction could not use as it is.
    pub path: String,
    /// What was wrong with it.
    pub issue: PathIssue,
    /// The file the chunk landed in, relative to the output directory.
    ///
    /// `None` when nothing was written, which is every issue but
    /// [`PathIssue::Refused`].
    pub output_path: Option<Utf8PathBuf>,
}

/// Where each extracted chunk lands under the output directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ExtractLayout {
    /// At its resolved path, with every directory of that path.
    #[default]
    Paths,
    /// In the output directory itself, by its file name alone.
    ///
    /// When two chunks of one extraction share a name, the second takes its
    /// path hash before the extension, as `name.<hash>.ext`. Which of the two
    /// is second follows write order.
    Flat,
}

/// What to do with a chunk whose file exists already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum ExistingFilePolicy {
    /// Write over it.
    #[default]
    Overwrite,
    /// Leave it, and count the chunk under [`ExtractReport::skipped_existing`].
    ///
    /// The worker opens the file with `create_new`, so it leaves a file that
    /// appears between two chunks alone too, and no check races the write.
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
    /// Chunks left out because their resolved path was unusable: it left the
    /// output directory, or another chunk claimed it first.
    ///
    /// [`displaced`](Self::displaced) says which, for each of them.
    pub skipped_unusable_path: usize,
    /// Path hashes given to [`extract_chunks`](WadExtractor::extract_chunks)
    /// that the archive holds no chunk for.
    pub missing: Vec<WadHash>,
    /// Bytes written, after decompression.
    pub bytes_written: u64,
    /// Written chunks, by the kind their bytes identify as.
    pub by_kind: BTreeMap<LeagueFileKind, usize>,
    /// The cancel flag was set, so the reader never reached some chunks.
    pub cancelled: bool,
    /// What [`with_name_recovery`](WadExtractor::with_name_recovery) read out
    /// of the archive's bins. Empty when recovery was off.
    pub recovered: RecoveredNames,
    /// Chunks that did not land at the path their resolver gave.
    pub displaced: Vec<DisplacedChunk>,
}

impl ExtractReport {
    fn record(&mut self, outcome: ChunkOutcome) {
        match outcome {
            ChunkOutcome::Written { kind, bytes } => {
                self.extracted += 1;
                self.bytes_written += bytes;
                *self.by_kind.entry(kind).or_insert(0) += 1;
            }
            ChunkOutcome::SkippedByType | ChunkOutcome::SkippedByPath => {
                self.skipped_by_filter += 1
            }
            ChunkOutcome::SkippedExisting => self.skipped_existing += 1,
            ChunkOutcome::SkippedUnwritablePath | ChunkOutcome::SkippedDuplicatePath => {
                self.skipped_unusable_path += 1
            }
        }
    }

    /// Count a chunk and, when it had one, note what was wrong with its path.
    fn record_chunk(&mut self, outcome: ChunkOutcome, displaced: Option<DisplacedChunk>) {
        self.record(outcome);
        if let Some(displaced) = displaced {
            self.displaced.push(displaced);
        }
    }

    /// Chunks written under a name that is not the one their path gave.
    fn renamed(&self) -> usize {
        self.displaced
            .iter()
            .filter(|chunk| chunk.issue == PathIssue::Refused)
            .count()
    }
}

impl fmt::Display for ExtractReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} extracted, {} bytes",
            self.extracted, self.bytes_written
        )?;
        if self.skipped_existing > 0 {
            write!(f, ", {} existed", self.skipped_existing)?;
        }
        if self.skipped_by_filter > 0 {
            write!(f, ", {} filtered out", self.skipped_by_filter)?;
        }
        if self.skipped_unusable_path > 0 {
            write!(f, ", {} unusable paths", self.skipped_unusable_path)?;
        }
        let renamed = self.renamed();
        if renamed > 0 {
            write!(f, ", {renamed} renamed")?;
        }
        if !self.missing.is_empty() {
            write!(f, ", {} missing", self.missing.len())?;
        }
        if !self.recovered.is_empty() {
            write!(f, ", {} names recovered", self.recovered.len())?;
        }
        if self.cancelled {
            f.write_str(", cancelled")?;
        }
        Ok(())
    }
}

/// What happened to one chunk, with the figures the report sums.
#[derive(Debug, Clone, Copy)]
enum ChunkOutcome {
    Written { kind: LeagueFileKind, bytes: u64 },
    SkippedByType,
    SkippedByPath,
    SkippedExisting,
    SkippedUnwritablePath,
    SkippedDuplicatePath,
}

impl ChunkOutcome {
    fn bytes(self) -> u64 {
        match self {
            Self::Written { bytes, .. } => bytes,
            Self::SkippedByType
            | Self::SkippedByPath
            | Self::SkippedExisting
            | Self::SkippedUnwritablePath
            | Self::SkippedDuplicatePath => 0,
        }
    }
}

/// What a worker did with one chunk, and the file it wrote for it.
struct WriteOutcome {
    outcome: ChunkOutcome,
    /// The file written, relative to the output directory, or `None` when
    /// nothing was written.
    path: Option<Utf8PathBuf>,
    /// What was wrong with the chunk's own path, when something was.
    issue: Option<PathIssue>,
}

impl WriteOutcome {
    /// A chunk that wrote nothing.
    fn skipped(outcome: ChunkOutcome, issue: Option<PathIssue>) -> Self {
        Self {
            outcome,
            path: None,
            issue,
        }
    }
}

impl From<ChunkOutcome> for ExtractResult {
    fn from(outcome: ChunkOutcome) -> Self {
        match outcome {
            ChunkOutcome::Written { .. } => ExtractResult::Extracted,
            ChunkOutcome::SkippedByType => ExtractResult::SkippedByType,
            ChunkOutcome::SkippedByPath => ExtractResult::SkippedByPath,
            ChunkOutcome::SkippedExisting => ExtractResult::SkippedExisting,
            ChunkOutcome::SkippedUnwritablePath => ExtractResult::SkippedUnwritablePath,
            ChunkOutcome::SkippedDuplicatePath => ExtractResult::SkippedDuplicatePath,
        }
    }
}

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
    cancel: Option<&'a AtomicBool>,
    workers: Option<NonZeroUsize>,
    recover_names: bool,
}

impl fmt::Debug for WadExtractor<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WadExtractor")
            .field("layout", &self.layout)
            .field("existing", &self.existing)
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
    /// An extraction asks the filter about each chunk **twice**: once to work
    /// out which paths it will write, and so which of them a directory has to
    /// hold, and once as it reads the chunk. A filter that only answers the
    /// question notices no difference; one that counts what it accepted, or
    /// drives a progress bar, counts each chunk twice.
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
    /// `output_dir`, in the order given.
    ///
    /// A hash given twice counts once. A hash the archive holds no chunk for
    /// lands under [`ExtractReport::missing`] and is not an error.
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

    /// The directories the chunks of one extraction name between them.
    ///
    /// This resolves every chunk a second time, which is what buys a rename
    /// that does not follow the order the chunks happen to be written in. The
    /// type filter cannot be applied here, since a chunk's kind is not known
    /// until its bytes are decompressed, so a chunk that filter goes on to drop
    /// still counts as a directory of the path it names.
    fn directory_paths(&self, chunks: &[WadChunk], resolver: &dyn PathResolver) -> DirectoryPaths {
        DirectoryPaths::of(chunks.iter().filter_map(|chunk| {
            let path = resolver.resolve(chunk.path_hash)?;
            /* A path the extraction refuses, or one the filter drops, is never
            written, so it makes no directory. */
            let written = !is_evil(&path)
                && self
                    .filter
                    .as_ref()
                    .is_none_or(|filter| filter(path.as_str()));
            written.then_some(path)
        }))
    }

    fn run<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        chunks: Vec<WadChunk>,
        missing: Vec<WadHash>,
        output_dir: &Utf8Path,
    ) -> Result<ExtractReport, WadError> {
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

        let directories = match self.layout {
            ExtractLayout::Paths => self.directory_paths(&chunks, &resolver),
            /* A flat layout writes file names into the output directory itself
            and makes no directory, so no path of it can be one. */
            ExtractLayout::Flat => DirectoryPaths::default(),
        };

        let shared = Shared {
            writer: ChunkWriter {
                layout: self.layout,
                existing: self.existing,
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
            filter: self.filter.as_deref(),
            cancel: self.cancel,
            progress: Progress {
                callback: self.progress.as_deref_mut(),
                done: 0,
                total: chunks.len(),
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

            let result = reader.read_chunks(wad, &chunks, &resolver, &sender, shared, &done);
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
            output_path: self.output_path.clone(),
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
    filter: Option<&'r (dyn Fn(&str) -> bool + 'a)>,
    cancel: Option<&'r AtomicBool>,
    progress: Progress<'r, 'a>,
}

impl Reader<'_, '_> {
    /// Feed the workers, chunk by chunk, and say whether the cancel flag stopped it.
    fn read_chunks<S: Read + Seek>(
        &mut self,
        wad: &mut Wad<S>,
        chunks: &[WadChunk],
        resolver: &dyn PathResolver,
        sender: &mpsc::SyncSender<Job>,
        shared: &Shared<'_>,
        done: &mpsc::Receiver<Done>,
    ) -> Result<bool, WadError> {
        for chunk in chunks {
            if self.cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
                return Ok(true);
            }
            if shared.failed() {
                return Ok(false);
            }

            let (path, named) = match resolver.resolve(chunk.path_hash) {
                Some(path) => (path, true),
                None => (hex_name(chunk.path_hash), false),
            };

            /* Before the filter, so a caller's selection cannot mask the fact
            that its resolver handed out a path the extraction will not write. */
            if is_evil(&path) {
                let finished = Done {
                    path_hash: chunk.path_hash,
                    path,
                    named,
                    output_path: None,
                    outcome: ChunkOutcome::SkippedUnwritablePath,
                };
                lock(&shared.report).record_chunk(
                    finished.outcome,
                    finished.displaced(Some(PathIssue::Unwritable)),
                );
                self.progress.report(&finished);
                continue;
            }

            if self.filter.is_some_and(|filter| !filter(path.as_str())) {
                let finished = Done {
                    path_hash: chunk.path_hash,
                    path,
                    named,
                    output_path: None,
                    outcome: ChunkOutcome::SkippedByPath,
                };
                lock(&shared.report).record_chunk(finished.outcome, None);
                self.progress.report(&finished);
                continue;
            }

            let raw = wad
                .load_chunk_raw(chunk)
                .map_err(|error| WadError::chunk(chunk.path_hash, &path, error))?;
            let job = Job {
                chunk: *chunk,
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

/// The half of the extractor that the workers share.
///
/// Everything here is [`Sync`]. The resolver, the path filter and the progress
/// callback stay on the reader, which is what keeps those three free of
/// any such bound.
struct ChunkWriter<'s> {
    layout: ExtractLayout,
    existing: ExistingFilePolicy,
    type_filter: Option<&'s [LeagueFileKind]>,
    output_dir: &'s Utf8Path,
    /* The directories this extraction's own paths name, known before any of
    them is written. */
    directories: DirectoryPaths,
    /* The names this extraction gave so far, so a second chunk claiming one of
    them can tell. Behind a mutex because the workers claim concurrently. */
    claimed: Mutex<HashSet<Utf8PathBuf>>,
}

impl ChunkWriter<'_> {
    fn write(&self, job: &Job, decoder: &mut ChunkDecoder) -> Result<WriteOutcome, WadError> {
        let data = decoder.decompress(
            &job.raw,
            job.chunk.compression_type,
            job.chunk.uncompressed_size,
        )?;
        self.write_chunk(&job.chunk, &data, Utf8Path::new(&job.path), job.named)
    }

    fn write_chunk(
        &self,
        chunk: &WadChunk,
        chunk_data: &[u8],
        chunk_path: &Utf8Path,
        named: bool,
    ) -> Result<WriteOutcome, WadError> {
        let chunk_kind = LeagueFileKind::identify_from_bytes(chunk_data);

        if self
            .type_filter
            .is_some_and(|types| !types.contains(&chunk_kind))
        {
            return Ok(WriteOutcome::skipped(ChunkOutcome::SkippedByType, None));
        }

        let (mut relative_path, mut issue) = match self.layout {
            ExtractLayout::Paths => {
                let (path, issue) = self.resolve_final_path(chunk, chunk_path, named, chunk_kind);
                /* Two hashes resolving to one path means the resolver is wrong
                about one of them. Keeping the first file and saying so beats
                letting the second overwrite it unseen. */
                if !lock(&self.claimed).insert(path.clone()) {
                    return Ok(WriteOutcome::skipped(
                        ChunkOutcome::SkippedDuplicatePath,
                        Some(PathIssue::Duplicate),
                    ));
                }
                (path, issue)
            }
            /* The flat layout gives a second chunk of one name its hash rather
            than drop it, because a flat tree collides by design. */
            ExtractLayout::Flat => self.resolve_flat_path(chunk, chunk_path, named, chunk_kind),
        };
        let full_path = self.output_dir.join(&relative_path);
        let written = match self.place(&relative_path, chunk_data) {
            Ok(written) => written,
            Err(error) if is_path_conflict(&error, &full_path) => {
                /* Something already occupies this path, so the chunk takes its
                hash in the output directory itself and loses the directories
                its path named. The report lists it and the name it landed
                under. */
                relative_path = hashed_name(chunk, chunk_kind);
                issue = Some(PathIssue::Refused);
                lock(&self.claimed).insert(relative_path.clone());
                self.place(&relative_path, chunk_data)?
            }
            Err(error) => return Err(WadError::IoError(error)),
        };

        let outcome = match written {
            Written::Yes => ChunkOutcome::Written {
                kind: chunk_kind,
                bytes: chunk_data.len() as u64,
            },
            Written::Existed => ChunkOutcome::SkippedExisting,
        };
        Ok(WriteOutcome {
            outcome,
            path: Some(relative_path),
            issue,
        })
    }

    /// Resolve the final output path for a chunk.
    /// Write `data` at `relative`, making the directories it names first.
    fn place(&self, relative: &Utf8Path, data: &[u8]) -> io::Result<Written> {
        let full_path = self.output_dir.join(relative);
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_file(&full_path, data, self.existing)
    }

    /// The file a chunk lands in, and what was wrong with the path it asked for.
    fn resolve_final_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        named: bool,
        chunk_kind: LeagueFileKind,
    ) -> (Utf8PathBuf, Option<PathIssue>) {
        let mut final_path = chunk_path.to_path_buf();

        /* A chunk no resolver named is here under its hash, and takes the
        extension its bytes identify as. */
        if !named {
            if let Some(ext) = chunk_kind.extension() {
                final_path.set_extension(ext);
            }
        }

        /* A directory holding the name leaves no choice: a file cannot share a
        name with one. Nothing else moves a chunk off the name its path, or its
        hash, gave it. The extraction knows the directories of its own paths
        before it writes any of them; one the output tree held already takes a
        look to find. */
        if !self.directories.holds(final_path.as_str())
            && !self.output_dir.join(&final_path).is_dir()
        {
            return (final_path, None);
        }

        let renamed = ltk_path(&final_path);
        /* A path can name the suffixed name a directory too. Nothing is left
        to suffix onto, since a second `.ltk` would no longer strip back to the
        path, so the chunk takes its hash: the name any refused write falls to.
        Deciding it here and not by failing the write is what keeps it off the
        order-dependent branch. */
        if self.directories.holds(renamed.as_str()) {
            return (hashed_name(chunk, chunk_kind), Some(PathIssue::Refused));
        }

        (renamed, Some(PathIssue::Refused))
    }

    /// The file name alone, made unique among the names this extraction wrote.
    fn resolve_flat_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        named: bool,
        chunk_kind: LeagueFileKind,
    ) -> (Utf8PathBuf, Option<PathIssue>) {
        let file_name = Utf8Path::new(chunk_path.file_name().unwrap_or_default());
        let (resolved, issue) = self.resolve_final_path(chunk, file_name, named, chunk_kind);

        let mut claimed = lock(&self.claimed);
        if claimed.insert(resolved.clone()) {
            return (resolved, issue);
        }

        let suffixed = Utf8PathBuf::from(match resolved.extension() {
            Some(ext) => format!(
                "{}.{:016x}.{ext}",
                resolved.file_stem().unwrap_or_default(),
                chunk.path_hash
            ),
            None => format!("{}.{:016x}", resolved.as_str(), chunk.path_hash),
        });
        claimed.insert(suffixed.clone());
        (suffixed, issue)
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
                /* A directory of this name is not a file left over from an
                earlier extraction, so it is not something to skip over. */
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists && !path.is_dir() => {
                    Ok(Written::Existed)
                }
                Err(error) => Err(error),
            }
        }
    }
}

/// Whether `error` says something already occupies `path`, rather than the
/// write having failed for a reason a different name would not mend.
///
/// Windows reports a directory in the way as `PermissionDenied`, and reports a
/// file it will not open at all the same way, so the path settles that one.
fn is_path_conflict(error: &io::Error, path: &Utf8Path) -> bool {
    use io::ErrorKind as Kind;

    match error.kind() {
        Kind::InvalidFilename | Kind::IsADirectory | Kind::AlreadyExists | Kind::NotADirectory => {
            true
        }
        Kind::PermissionDenied => path.is_dir(),
        _ => false,
    }
}

/// `<hash>.<ext>`, the name a chunk takes when the file system refuses its own.
fn hashed_name(chunk: &WadChunk, chunk_kind: LeagueFileKind) -> Utf8PathBuf {
    let mut hashed_path = Utf8PathBuf::from(hex_name(chunk.path_hash));
    if let Some(ext) = chunk_kind.extension() {
        hashed_path.set_extension(ext);
    }
    hashed_path
}

/// Check if a path looks like a hex-encoded hash (e.g., "0123456789abcdef").
///
/// This is the name a chunk gets on disk when nothing resolves its hash, so
/// a caller can sort a file tree extracted earlier into named and unnamed
/// files.
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

/// `<name>.ltk`, the name a chunk takes when a directory holds its own.
///
/// The suffix is added and never substituted, so stripping a trailing `.ltk`
/// gives back the path the chunk was named for, whatever that path held. A
/// caller that hashes an extracted file's path back to its chunk needs exactly
/// that, and a name built from the file's stem could not give it: `foo.bin`
/// renamed to `foo.ltk.dds` says nothing about the `.bin` it came from.
fn ltk_name(file_name: &str) -> String {
    format!("{file_name}.ltk")
}

/// `path` with the suffix on the end of its file name.
///
/// The suffix goes on the end of the path, which is the same thing: a path's
/// file name is its tail. Going through
/// [`set_file_name`](Utf8PathBuf::set_file_name) would not be, because that
/// re-joins the path with the host's separator and would hand back
/// `assets\thing.ltk` on Windows where every un-renamed chunk reports
/// `assets/thing`. A caller stripping the suffix off that would hash a path
/// the archive was never built from.
fn ltk_path(path: &Utf8Path) -> Utf8PathBuf {
    Utf8PathBuf::from(ltk_name(path.as_str()))
}
