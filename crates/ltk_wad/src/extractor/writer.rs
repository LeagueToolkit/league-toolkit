//! Putting one chunk's bytes on disk.
//!
//! [`ChunkWriter`] is the half of the extractor the workers share, so
//! everything here is [`Sync`]. What stays on the reader thread instead, and
//! why, is in the extractor module docs, under "Parallelism".

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write as _},
    sync::Mutex,
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;

use crate::{ChunkDecoder, SubchunkToc, WadChunk, WadError};

use super::{
    lock,
    naming::{hashed_name, is_path_conflict, ltk_path, DirectoryPaths, NamingPolicy},
    report::{ChunkOutcome, WriteOutcome},
    Job, PathIssue,
};

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
    /// Leave it, and count the chunk under [`ExtractReport::skipped_existing`](crate::ExtractReport::skipped_existing).
    ///
    /// The worker opens the file with `create_new`, so it leaves a file that
    /// appears between two chunks alone too, and no check races the write.
    Skip,
}

/// The half of the extractor that the workers share.
///
/// Everything here is [`Sync`]. What stays on the reader thread instead, and
/// why, is in the extractor module docs, under "Parallelism".
pub(super) struct ChunkWriter<'s> {
    pub(super) layout: ExtractLayout,
    pub(super) existing: ExistingFilePolicy,
    pub(super) naming: NamingPolicy,
    pub(super) type_filter: Option<&'s [LeagueFileKind]>,
    pub(super) output_dir: &'s Utf8Path,
    /* The directories this extraction's own paths name, known before any of
    them is written. */
    pub(super) directories: DirectoryPaths,
    /* Whether the output directory held anything before the extraction. */
    pub(super) output_occupied: bool,
    /* The names this extraction gave so far, so a second chunk claiming one of
    them can tell. Behind a mutex because the workers claim concurrently. */
    pub(super) claimed: Mutex<HashSet<Utf8PathBuf>>,
    /* The directories made so far, so each is made once. */
    pub(super) created: Mutex<HashSet<Utf8PathBuf>>,
    /* The archive's subchunk table, when it has one. */
    pub(super) subchunk_toc: Option<SubchunkToc>,
}

impl ChunkWriter<'_> {
    pub(super) fn write(
        &self,
        job: &Job,
        decoder: &mut ChunkDecoder,
    ) -> Result<WriteOutcome, WadError> {
        let data = decoder.decompress_chunk(&job.raw, &job.chunk, self.subchunk_toc.as_ref())?;
        self.write_chunk(&job.chunk, &data, Utf8Path::new(&job.path), job.named)
    }

    /// The skip a chunk gets from its name alone, before its bytes are read.
    ///
    /// `Some` claims the path and skips the chunk; `None` sends it down the
    /// ordinary write path.
    pub(super) fn probe_skip(
        &self,
        chunk: &WadChunk,
        path: &str,
        named: bool,
    ) -> Option<WriteOutcome> {
        if self.existing != ExistingFilePolicy::Skip
            || self.type_filter.is_some()
            || self.layout != ExtractLayout::Paths
        {
            return None;
        }
        let (relative, renamed) = self.final_path(chunk, Utf8Path::new(path), named, None)?;

        let mut claimed = lock(&self.claimed);
        if claimed.contains(&relative) {
            return None;
        }
        if !self.output_dir.join(&relative).is_file() {
            return None;
        }
        claimed.insert(relative.clone());

        let issue = renamed.then(|| PathIssue::Renamed(relative.clone()));
        Some(WriteOutcome {
            outcome: ChunkOutcome::SkippedExisting,
            path: Some(relative),
            issue,
        })
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

        let (mut relative_path, mut renamed) = match self.layout {
            ExtractLayout::Paths => {
                let (path, renamed) = self.resolve_final_path(chunk, chunk_path, named, chunk_kind);
                match self.claim_path(path, renamed, chunk, chunk_kind) {
                    Some(claimed) => claimed,
                    None => {
                        return Ok(WriteOutcome::skipped(
                            ChunkOutcome::SkippedDuplicatePath,
                            Some(PathIssue::Duplicate),
                        ))
                    }
                }
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
                relative_path = hashed_name(chunk, chunk_kind, self.naming);
                renamed = true;
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
        let issue = renamed.then(|| PathIssue::Renamed(relative_path.clone()));
        Ok(WriteOutcome {
            outcome,
            path: Some(relative_path),
            issue,
        })
    }

    /// Write `data` at `relative`, making the directories it names first.
    fn place(&self, relative: &Utf8Path, data: &[u8]) -> io::Result<Written> {
        let full_path = self.output_dir.join(relative);
        if let Some(parent) = full_path.parent() {
            self.create_dirs(parent)?;
        }
        write_file(&full_path, data, self.existing)
    }

    /// Recursively creates the ancestor path directories, each once per extraction.
    fn create_dirs(&self, parent: &Utf8Path) -> io::Result<()> {
        if lock(&self.created).contains(parent) {
            return Ok(());
        }
        fs::create_dir_all(parent)?;
        let mut created = lock(&self.created);
        for ancestor in parent.ancestors() {
            /* Stop above the output directory, or at an ancestor already noted. */
            if !ancestor.starts_with(self.output_dir) || !created.insert(ancestor.to_path_buf()) {
                break;
            }
        }
        Ok(())
    }

    /// The file a chunk lands in, and whether that is a name its path did not give.
    fn resolve_final_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        named: bool,
        chunk_kind: LeagueFileKind,
    ) -> (Utf8PathBuf, bool) {
        self.final_path(chunk, chunk_path, named, Some(chunk_kind))
            .expect("a path resolves whenever the chunk's kind is known")
    }

    /// The file a chunk lands in, or `None` when only the chunk's bytes can
    /// say and `chunk_kind` is not known yet.
    fn final_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        named: bool,
        chunk_kind: Option<LeagueFileKind>,
    ) -> Option<(Utf8PathBuf, bool)> {
        let mut final_path = chunk_path.to_path_buf();

        /* A nameless chunk is here under its hash. */
        if !named && self.naming == NamingPolicy::Descriptive {
            if let Some(ext) = chunk_kind?.extension() {
                final_path.set_extension(ext);
            }
        }

        /* Our own directories are known up front; a pre-existing one takes a
        look to find, which a pristine output cannot need. */
        if !self.directories.holds(final_path.as_str())
            && !(self.output_occupied && self.output_dir.join(&final_path).is_dir())
        {
            return Some((final_path, false));
        }

        let renamed = ltk_path(&final_path);
        /* The suffixed name can be a directory too. Nothing is left to suffix
        onto, so the chunk takes its hash. */
        if self.directories.holds(renamed.as_str()) {
            let kind = match self.naming {
                /* The lossless hash name invents no extension, so it needs no
                kind. */
                NamingPolicy::Lossless => chunk_kind.unwrap_or(LeagueFileKind::Unknown),
                _ => chunk_kind?,
            };
            return Some((hashed_name(chunk, kind, self.naming), true));
        }

        Some((renamed, true))
    }

    /// The name this chunk keeps, or `None` when another chunk holds it and
    /// the policy leaves this one unwritten.
    ///
    /// Two hashes resolving to one path means the resolver is wrong about one
    /// of them, and the policy says what the second chunk gets:
    ///
    /// - [`NamingPolicy::Descriptive`]: nothing. The first file stands and the
    ///   second is dropped, which beats letting it overwrite the first unseen.
    /// - [`NamingPolicy::Lossless`]: a `.ltk` suffix, so both come through and
    ///   either path can be read back. A third chunk on the same path takes
    ///   its hash, which no other chunk can hold.
    fn claim_path(
        &self,
        path: Utf8PathBuf,
        renamed: bool,
        chunk: &WadChunk,
        chunk_kind: LeagueFileKind,
    ) -> Option<(Utf8PathBuf, bool)> {
        let mut claimed = lock(&self.claimed);
        if claimed.insert(path.clone()) {
            return Some((path, renamed));
        }
        if self.naming != NamingPolicy::Lossless {
            return None;
        }

        let suffixed = ltk_path(&path);
        if claimed.insert(suffixed.clone()) {
            return Some((suffixed, true));
        }

        /* A third chunk on one path, so the suffix is taken too. A chunk's
        hash is its own, so this always lands. */
        let hashed = hashed_name(chunk, chunk_kind, self.naming);
        claimed.insert(hashed.clone());
        Some((hashed, true))
    }

    /// The file name alone, made unique among the names this extraction wrote.
    fn resolve_flat_path(
        &self,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        named: bool,
        chunk_kind: LeagueFileKind,
    ) -> (Utf8PathBuf, bool) {
        let file_name = Utf8Path::new(chunk_path.file_name().unwrap_or_default());
        let (resolved, renamed) = self.resolve_final_path(chunk, file_name, named, chunk_kind);

        let mut claimed = lock(&self.claimed);
        if claimed.insert(resolved.clone()) {
            return (resolved, renamed);
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
        (suffixed, renamed)
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
