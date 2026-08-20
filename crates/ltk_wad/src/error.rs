use std::io;

use thiserror::Error;

/// What can go wrong with a WAD archive.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum WadError {
    /// The file does not start with the WAD magic.
    #[error("invalid header")]
    InvalidHeader { expected: String, actual: String },

    /// The file is a WAD of a version this crate does not read.
    #[error("invalid version {major:?}.{minor:?}")]
    InvalidVersion { major: u8, minor: u8 },

    /// A chunk names a compression this crate does not know.
    #[error("invalid chunk compression: {compression:?}")]
    InvalidChunkCompression { compression: u8 },

    /// Two chunks share one path hash.
    #[error("duplicate chunk: {path_hash:#08x}")]
    DuplicateChunk { path_hash: u64 },

    /// A chunk's bytes do not decompress.
    #[error("failed to decompress chunk: {reason}")]
    DecompressionFailure { reason: String },

    /// The source or the output could not be read or written.
    #[error("io error: {0}")]
    IoError(#[from] io::Error),

    /// A failure of one chunk, with the chunk it was.
    ///
    /// The extractor and the name recovery wrap every per-chunk failure in
    /// this, so an error out of an archive of thousands names its chunk. The
    /// path is the resolver's, or the hash as sixteen hex digits.
    #[error("chunk {path_hash:016x} ({path}): {source}")]
    Chunk {
        path_hash: u64,
        path: String,
        #[source]
        source: Box<WadError>,
    },

    /// Anything else.
    #[error("error: `{0}`")]
    Other(String),
}

impl WadError {
    /// `source`, as the failure of the chunk at `path`.
    pub(crate) fn chunk(path_hash: u64, path: &str, source: WadError) -> Self {
        Self::Chunk {
            path_hash,
            path: path.to_owned(),
            source: Box::new(source),
        }
    }
}
