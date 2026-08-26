//! Turning a resolved path into a name a file system will take.
//!
//! Three things stand between a path a resolver gave and a file on disk: the
//! path may be one the extraction must refuse outright ([`is_evil`]), another
//! path of the same extraction may need it to be a directory
//! ([`DirectoryPaths`]), or the file system may refuse the name when the write
//! is tried ([`is_path_conflict`]). The first two are settled before anything
//! is written, which is what makes one archive and one hash table give one
//! output tree on every run.
//!
//! `docs/design/wad-extractor.md` records why, and what was measured.

use std::{borrow::Cow, collections::HashSet, io};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;
use ltk_hash::WadHash;

use crate::WadChunk;

use ltk_hash::Hash as _;

use super::resolver::{hex_chunk_hash, hex_name};

/// Whether an extraction's names can be read back as the paths they came from.
///
/// The policy picks between naming a chunk for what its bytes are and keeping
/// every chunk under a name its resolved path can be read out of. The extractor
/// module docs, under "How a chunk is named on disk", say what each name is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum NamingPolicy {
    /// Names say what a file holds, at the price of dropping a duplicate.
    ///
    /// - A nameless chunk takes the extension its bytes identify as:
    ///   `<hash>.<ext>`.
    /// - A chunk whose path another chunk claimed first is left unwritten.
    #[default]
    Descriptive,
    /// Every chunk lands, under a name its resolved path can be read out of.
    ///
    /// - A nameless chunk lands under its bare hash, with no invented
    ///   extension.
    /// - A chunk whose path is taken appends `.ltk` rather than give up its
    ///   bytes. Stripping the suffix gives back the path the resolver named,
    ///   which is what a caller hashing an extracted file's path back to its
    ///   chunk needs.
    /// - A name the file system refuses outright still falls back to the
    ///   chunk's hash: a suffix only makes a name the host already rejected
    ///   longer.
    Lossless,
}

/// Whether `path` is one an extraction must refuse to write.
///
/// A resolver's paths are untrusted, a hash table and name recovery alike;
/// the extractor module docs say why, under "Paths the extraction will not
/// write". A path is evil when joining it onto the output directory would not
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
pub(super) fn is_evil(path: &str) -> bool {
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
pub(super) struct DirectoryPaths(HashSet<String>);

impl DirectoryPaths {
    /// The directories `paths` names.
    pub(super) fn of(paths: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
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
    pub(super) fn holds(&self, path: &str) -> bool {
        self.0.contains(plain_path(path).as_ref())
    }
}

/// `path` with one `/` between its components and nothing else.
///
/// The components are the ones a join steps onto, so an empty one from `a//b`
/// or from a trailing separator is dropped, as is a `.`, and `\` separates as
/// `/` does whatever the host. Borrowed when `path` is in that form already,
/// which nearly every path a hash table holds is.
pub(super) fn plain_path(path: &str) -> Cow<'_, str> {
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

/// Whether `error` says something already occupies `path`, rather than the
/// write having failed for a reason a different name would not mend.
///
/// Windows reports a directory standing in the way as `PermissionDenied`, and
/// reports a file it cannot open the same way, so `path.is_dir()` breaks the
/// tie between the two.
pub(super) fn is_path_conflict(error: &io::Error, path: &Utf8Path) -> bool {
    use io::ErrorKind as Kind;

    match error.kind() {
        Kind::InvalidFilename | Kind::IsADirectory | Kind::AlreadyExists | Kind::NotADirectory => {
            true
        }
        Kind::PermissionDenied => path.is_dir(),
        _ => false,
    }
}

/// The name a chunk takes when the file system refuses its own.
///
/// `<hash>.<ext>` under [`NamingPolicy::Descriptive`], and the bare `<hash>`
/// under [`NamingPolicy::Lossless`], which invents no extension.
pub(super) fn hashed_name(
    chunk: &WadChunk,
    chunk_kind: LeagueFileKind,
    naming: NamingPolicy,
) -> Utf8PathBuf {
    let mut hashed_path = Utf8PathBuf::from(hex_name(chunk.path_hash));
    if naming == NamingPolicy::Descriptive {
        if let Some(ext) = chunk_kind.extension() {
            hashed_path.set_extension(ext);
        }
    }
    hashed_path
}

/// `<name>.ltk`, the name a chunk takes when a directory holds its own.
///
/// The suffix is added and never substituted, so stripping a trailing `.ltk`
/// gives back the path the chunk was named for, whatever that path held. A
/// caller that hashes an extracted file's path back to its chunk needs exactly
/// that, and a name built from the file's stem could not give it: `foo.bin`
/// renamed to `foo.ltk.dds` says nothing about the `.bin` it came from.
pub(super) fn ltk_name(file_name: &str) -> String {
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
pub(super) fn ltk_path(path: &Utf8Path) -> Utf8PathBuf {
    Utf8PathBuf::from(ltk_name(path.as_str()))
}

/// `path` without the `.ltk` suffix an extraction adds, if it has one.
///
/// The exact inverse of the rename: the suffix is only ever added, never
/// substituted, so taking it off gives back the name the chunk was written
/// for. Borrowed when there is no suffix to take off.
pub fn strip_ltk_suffix(path: &Utf8Path) -> &Utf8Path {
    Utf8Path::new(path.as_str().strip_suffix(".ltk").unwrap_or(path.as_str()))
}

/// The chunk an extracted file was written for.
///
/// Reads the extraction's naming back: the `.ltk` suffix comes off, a hash
/// name parses as itself, and anything else is the path a resolver gave,
/// hashed as the archive keys it. Both naming policies read back the same
/// way, so this takes none.
///
/// The one shape it cannot tell apart is a resolver that named a chunk with
/// sixteen hex digits of its own, which reads as that hash rather than as the
/// path. [`ExtractProgress::is_named`](crate::ExtractProgress::is_named)
/// reports which a chunk was at the time it was written, for a caller that
/// must know.
///
/// # Example
///
/// ```
/// use ltk_wad::{chunk_hash_of, WadHash};
/// use camino::Utf8Path;
///
/// // A hash name, and the same chunk after a directory took its name.
/// let bare = chunk_hash_of(Utf8Path::new("0123456789abcdef"));
/// assert_eq!(bare, WadHash(0x0123456789abcdef));
/// assert_eq!(chunk_hash_of(Utf8Path::new("0123456789abcdef.ltk")), bare);
///
/// // A path a resolver gave reads back the same before and after a rename.
/// let named = chunk_hash_of(Utf8Path::new("assets/thing.bin"));
/// assert_eq!(chunk_hash_of(Utf8Path::new("assets/thing.bin.ltk")), named);
/// ```
pub fn chunk_hash_of(path: &Utf8Path) -> WadHash {
    let named = strip_ltk_suffix(path);
    match hex_chunk_hash(named) {
        Some(hash) => hash,
        None => WadHash::hash_str(plain_path(named.as_str()).as_ref()),
    }
}
