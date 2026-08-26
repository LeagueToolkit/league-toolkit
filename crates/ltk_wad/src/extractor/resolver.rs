//! Naming a chunk from its path hash.
//!
//! A WAD stores the hash of each chunk's path and not the path itself, so an
//! extraction needs something to supply the path. That is a [`PathResolver`],
//! and a chunk no resolver names falls back to its hash written out as sixteen
//! hex digits.

use std::{collections::HashMap, hash::BuildHasher, sync::Arc};

use camino::Utf8Path;
use ltk_hash::WadHash;

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

    /// The paths of `path_hashes`, one answer per hash, in the order given.
    ///
    /// An extraction asks for every chunk of an archive at once, so a resolver
    /// reading a compressed store can answer the batch in one pass over it
    /// rather than seeking per hash. The default calls
    /// [`resolve`](Self::resolve) once per hash, which is already what a
    /// resolver backed by a map costs.
    ///
    /// Overriding this must not change *what* is resolved. A hash answered
    /// here and a hash answered by [`resolve`](Self::resolve) name the same
    /// path.
    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        path_hashes.iter().map(|&hash| self.resolve(hash)).collect()
    }
}

impl<R: PathResolver + ?Sized> PathResolver for &R {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }

    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        (**self).resolve_all(path_hashes)
    }
}

impl<R: PathResolver + ?Sized> PathResolver for Box<R> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }

    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        (**self).resolve_all(path_hashes)
    }
}

impl<R: PathResolver + ?Sized> PathResolver for Arc<R> {
    fn resolve(&self, path_hash: WadHash) -> Option<String> {
        (**self).resolve(path_hash)
    }

    fn is_known(&self, path_hash: WadHash) -> bool {
        (**self).is_known(path_hash)
    }

    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        (**self).resolve_all(path_hashes)
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

/// Every hash answered, with the count the trait promises enforced.
///
/// # Panics
///
/// Panics when `resolver` answers a different number of hashes than it was
/// asked. Callers zip the answers back onto their chunks, where a short answer
/// would drop chunks from an extraction rather than fail it.
pub(crate) fn resolve_all_checked<R: PathResolver + ?Sized>(
    resolver: &R,
    path_hashes: &[WadHash],
) -> Vec<Option<String>> {
    let resolved = resolver.resolve_all(path_hashes);
    assert_eq!(
        resolved.len(),
        path_hashes.len(),
        "PathResolver::resolve_all answered {} of {} hashes",
        resolved.len(),
        path_hashes.len()
    );
    resolved
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

    fn resolve_all(&self, path_hashes: &[WadHash]) -> Vec<Option<String>> {
        vec![None; path_hashes.len()]
    }
}

/// A path hash as the sixteen hex digits a nameless chunk lands under.
///
/// Zero padded to sixteen digits, which is the width
/// [`hex_chunk_hash`] reads back and the only width a chunk name is valid at.
/// [`WadHash`]'s own `Display` pads to nothing, so `hash.to_string()` is not
/// this.
///
/// # Example
///
/// ```
/// use ltk_wad::{hex_name, WadHash};
///
/// assert_eq!(hex_name(WadHash(0xff)), "00000000000000ff");
/// ```
pub fn hex_name(path_hash: WadHash) -> String {
    format!("{path_hash:016x}")
}

/// The chunk `path` was written for, when its file stem is a hash.
///
/// The sixteen hex digits of a nameless chunk's name, read back. Returns
/// `None` for any other shape, so a caller sorting a file tree extracted
/// earlier can tell a hash name from a path a resolver gave.
///
/// # Example
///
/// ```
/// use ltk_wad::{hex_chunk_hash, WadHash};
/// use camino::Utf8Path;
///
/// let hash = hex_chunk_hash(Utf8Path::new("0123456789abcdef.bin"));
/// assert_eq!(hash, Some(WadHash(0x0123456789abcdef)));
/// assert_eq!(hex_chunk_hash(Utf8Path::new("assets/aatrox.bin")), None);
/// ```
pub fn hex_chunk_hash(path: &Utf8Path) -> Option<WadHash> {
    let file_stem = path.file_stem().unwrap_or("");
    if file_stem.len() != 16 || !file_stem.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    WadHash::from_str_radix(file_stem, 16).ok()
}

/// Whether `path`'s file stem is the sixteen hex digits a nameless chunk lands under.
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
    hex_chunk_hash(path).is_some()
}
