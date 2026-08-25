//! Putting one chunk's bytes on disk.
//!
//! [`ChunkWriter`] is the half of the extractor the workers share, so
//! everything here is [`Sync`]. The resolver, the path filter and the progress
//! callback stay on the reader thread, which is what keeps those three free of
//! any such bound.

use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io::{self, Write as _},
    sync::Mutex,
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;

use crate::{ChunkDecoder, WadChunk, WadError};

use super::{
    lock,
    naming::{hashed_name, is_path_conflict, ltk_path, DirectoryPaths},
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
/// Everything here is [`Sync`]. The resolver, the path filter and the progress
/// callback stay on the reader, which is what keeps those three free of
/// any such bound.
pub(super) struct ChunkWriter<'s> {
    pub(super) layout: ExtractLayout,
    pub(super) existing: ExistingFilePolicy,
    pub(super) type_filter: Option<&'s [LeagueFileKind]>,
    pub(super) output_dir: &'s Utf8Path,
    /* The directories this extraction's own paths name, known before any of
    them is written. */
    pub(super) directories: DirectoryPaths,
    /* The names this extraction gave so far, so a second chunk claiming one of
    them can tell. Behind a mutex because the workers claim concurrently. */
    pub(super) claimed: Mutex<HashSet<Utf8PathBuf>>,
}

impl ChunkWriter<'_> {
    pub(super) fn write(
        &self,
        job: &Job,
        decoder: &mut ChunkDecoder,
    ) -> Result<WriteOutcome, WadError> {
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
