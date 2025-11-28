//! WAD chunk extraction utilities.
//!
//! This module provides abstractions for extracting chunks from WAD archives to disk.
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use std::collections::HashMap;
//! use std::borrow::Cow;
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
//!     let (mut decoder, chunks) = wad.decode();
//!     let extracted = extractor.extract_all(&mut decoder, chunks, "/output/path")?;
//!     println!("Extracted {} chunks", extracted);
//!
//!     Ok(())
//! }
//! ```

use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    io::{self, Read, Seek},
};

use camino::{Utf8Path, Utf8PathBuf};
use ltk_file::LeagueFileKind;

use crate::{WadChunk, WadDecoder, WadError};

/// A trait for resolving path hashes to human-readable paths.
///
/// Implement this trait to provide path resolution from a hashtable or other source.
pub trait PathResolver {
    /// Resolve a path hash to a path string.
    ///
    /// If the hash cannot be resolved, implementations should return the hash
    /// formatted as a hex string (e.g., `format!("{:016x}", path_hash)`).
    fn resolve(&self, path_hash: u64) -> Cow<'_, str>;
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
}

/// Type alias for the progress callback function.
pub type ProgressCallback<'a> = Box<dyn Fn(ExtractProgress<'_>) + 'a>;

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

    /// Extract all chunks from the decoder to the specified directory.
    ///
    /// Returns the number of chunks actually extracted (not skipped).
    pub fn extract_all<TSource: Read + Seek>(
        &self,
        decoder: &mut WadDecoder<'_, TSource>,
        chunks: &HashMap<u64, WadChunk>,
        output_dir: impl AsRef<Utf8Path>,
    ) -> Result<usize, WadError> {
        let output_dir = output_dir.as_ref();
        let total = chunks.len();
        let mut extracted_count = 0;

        for (index, chunk) in chunks.values().enumerate() {
            let chunk_path_str = self.resolver.resolve(chunk.path_hash());
            let chunk_path = Utf8Path::new(chunk_path_str.as_ref());

            // Report progress
            if let Some(ref callback) = self.progress_callback {
                callback(ExtractProgress {
                    current: index,
                    total,
                    current_path: chunk_path_str.as_ref(),
                    path_hash: chunk.path_hash(),
                });
            }

            // Check path filter
            if let Some(ref filter) = self.filter {
                if !filter.matches(chunk_path_str.as_ref()) {
                    continue;
                }
            }

            // Extract the chunk
            match self.extract_chunk(decoder, chunk, chunk_path, output_dir)? {
                ExtractResult::Extracted => extracted_count += 1,
                ExtractResult::SkippedByType | ExtractResult::SkippedByPattern => {}
            }
        }

        Ok(extracted_count)
    }

    /// Extract a single chunk to the specified directory.
    ///
    /// Returns the extraction result indicating whether the chunk was extracted or skipped.
    pub fn extract_chunk<TSource: Read + Seek>(
        &self,
        decoder: &mut WadDecoder<'_, TSource>,
        chunk: &WadChunk,
        chunk_path: &Utf8Path,
        output_dir: &Utf8Path,
    ) -> Result<ExtractResult, WadError> {
        // Decompress the chunk data
        let chunk_data = decoder.load_chunk_decompressed(chunk)?;

        // Identify the file type
        let chunk_kind = LeagueFileKind::identify_from_bytes(&chunk_data);

        // Check type filter
        if let Some(ref type_filter) = self.type_filter {
            if !type_filter.contains(&chunk_kind) {
                return Ok(ExtractResult::SkippedByType);
            }
        }

        // Determine the final output path
        let final_path = self.resolve_final_path(chunk_path, output_dir, &chunk_data, chunk_kind);
        let full_path = output_dir.join(&final_path);

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write the file
        match fs::write(&full_path, &chunk_data) {
            Ok(()) => Ok(ExtractResult::Extracted),
            Err(error) if error.kind() == io::ErrorKind::InvalidFilename => {
                // Fallback for long filenames
                self.write_with_hashed_name(chunk, &chunk_data, chunk_kind, output_dir)?;
                Ok(ExtractResult::Extracted)
            }
            Err(error) => Err(WadError::IoError(error)),
        }
    }

    /// Resolve the final output path for a chunk.
    fn resolve_final_path(
        &self,
        chunk_path: &Utf8Path,
        output_dir: &Utf8Path,
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
        let collides_with_dir = output_dir.join(&final_path).is_dir();
        if !has_extension || collides_with_dir {
            final_path.set_file_name(build_ltk_name(
                chunk_path.file_stem().unwrap_or_default(),
                chunk_data,
            ));
        }

        final_path
    }

    /// Write a chunk with a hashed filename (fallback for long filenames).
    fn write_with_hashed_name(
        &self,
        chunk: &WadChunk,
        chunk_data: &[u8],
        chunk_kind: LeagueFileKind,
        output_dir: &Utf8Path,
    ) -> Result<(), WadError> {
        let mut hashed_path = Utf8PathBuf::from(format!("{:016x}", chunk.path_hash()));
        if let Some(ext) = chunk_kind.extension() {
            hashed_path.set_extension(ext);
        }

        fs::write(output_dir.join(hashed_path), chunk_data)?;
        Ok(())
    }
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
///
/// assert!(is_hex_chunk_path("0123456789abcdef"));
/// assert!(is_hex_chunk_path("0123456789abcdef.bin"));
/// assert!(!is_hex_chunk_path("assets/champions/aatrox.bin"));
/// ```
pub fn is_hex_chunk_path(path: &Utf8Path) -> bool {
    let file_name = path.file_name().unwrap_or("");
    file_name.len() == 16 && file_name.chars().all(|c| c.is_ascii_hexdigit())
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
    fn test_hex_path_resolver() {
        let resolver = HexPathResolver;
        assert_eq!(resolver.resolve(0x0123456789abcdef), "0123456789abcdef");
    }

    #[test]
    fn test_hashmap_path_resolver() {
        let mut resolver = HashMapPathResolver::new(HashMap::new());
        resolver.insert(0x1234, "assets/test.bin".to_string());

        assert_eq!(resolver.resolve(0x1234), "assets/test.bin");
        assert_eq!(resolver.resolve(0x5678), "0000000000005678");
    }

    #[test]
    fn test_build_ltk_name() {
        // Unknown type
        assert_eq!(build_ltk_name("myfile", &[]), "myfile.ltk");

        // PNG magic bytes
        let png_magic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(build_ltk_name("myfile", &png_magic), "myfile.ltk.png");
    }

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
}
