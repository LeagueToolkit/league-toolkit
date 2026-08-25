//! What an extraction did, chunk by chunk and in sum.
//!
//! [`ExtractProgress`] reports one chunk as it finishes and [`ExtractReport`]
//! sums the run. A chunk that did not land at the path its resolver gave is
//! listed under [`ExtractReport::displaced`], with a [`PathIssue`] saying why
//! and the file it landed in instead, so no rename and no dropped chunk is
//! silent.

use std::{collections::BTreeMap, fmt};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;
use ltk_hash::WadHash;

use crate::RecoveredNames;

/// One chunk done, as [`WadExtractor::on_progress`](crate::WadExtractor::on_progress) reports it.
#[derive(Debug, Clone)]
pub struct ExtractProgress<'a> {
    pub(super) done: usize,
    pub(super) total: usize,
    pub(super) path_hash: WadHash,
    pub(super) path: &'a str,
    pub(super) named: bool,
    pub(super) output_path: Option<&'a Utf8Path>,
    pub(super) result: ExtractResult,
    pub(super) bytes: u64,
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
    /// rather than [`is_hex_chunk_path`](crate::is_hex_chunk_path), which cannot tell a real name of
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
    /// Its file existed already, and the policy was [`ExistingFilePolicy::Skip`](crate::ExistingFilePolicy::Skip).
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
    /// download, and [`with_name_recovery`](crate::WadExtractor::with_name_recovery)
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

/// What an extraction did, summed over its chunks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExtractReport {
    /// Chunks written to disk.
    pub extracted: usize,
    /// Chunks left alone under [`ExistingFilePolicy::Skip`](crate::ExistingFilePolicy::Skip).
    pub skipped_existing: usize,
    /// Chunks the path filter or the type filter left out.
    pub skipped_by_filter: usize,
    /// Chunks left out because their resolved path was unusable: it left the
    /// output directory, or another chunk claimed it first.
    ///
    /// [`displaced`](Self::displaced) says which, for each of them.
    pub skipped_unusable_path: usize,
    /// Path hashes given to [`extract_chunks`](crate::WadExtractor::extract_chunks)
    /// that the archive holds no chunk for.
    pub missing: Vec<WadHash>,
    /// Bytes written, after decompression.
    pub bytes_written: u64,
    /// Written chunks, by the kind their bytes identify as.
    pub by_kind: BTreeMap<LeagueFileKind, usize>,
    /// The cancel flag was set, so the reader never reached some chunks.
    pub cancelled: bool,
    /// What [`with_name_recovery`](crate::WadExtractor::with_name_recovery) read out
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
    pub(super) fn record_chunk(
        &mut self,
        outcome: ChunkOutcome,
        displaced: Option<DisplacedChunk>,
    ) {
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
pub(super) enum ChunkOutcome {
    Written { kind: LeagueFileKind, bytes: u64 },
    SkippedByType,
    SkippedByPath,
    SkippedExisting,
    SkippedUnwritablePath,
    SkippedDuplicatePath,
}

impl ChunkOutcome {
    pub(super) fn bytes(self) -> u64 {
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
pub(super) struct WriteOutcome {
    pub(super) outcome: ChunkOutcome,
    /// The file written, relative to the output directory, or `None` when
    /// nothing was written.
    pub(super) path: Option<Utf8PathBuf>,
    /// What was wrong with the chunk's own path, when something was.
    pub(super) issue: Option<PathIssue>,
}

impl WriteOutcome {
    /// A chunk that wrote nothing.
    pub(super) fn skipped(outcome: ChunkOutcome, issue: Option<PathIssue>) -> Self {
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
