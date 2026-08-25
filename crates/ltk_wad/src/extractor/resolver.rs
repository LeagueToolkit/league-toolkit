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
