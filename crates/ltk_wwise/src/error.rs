use std::io;

use crate::package::PackageKey;

/// The result type of `ltk_wwise`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error from reading, decoding, encoding or writing a Wwise file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    /// The first bytes are not the magic of the format.
    #[error("Invalid magic: expected {expected:?}, found {found:?}")]
    InvalidMagic { expected: [u8; 4], found: [u8; 4] },

    /// The version of a package is not 1.
    #[error("unsupported package version {0}")]
    PackageVersion(u32),

    /// Two entries of a package have one key.
    #[error("two package entries have the key {0:?}")]
    DuplicateKey(PackageKey),

    /// An entry record or a media ends after the end of the package.
    ///
    /// `name` is the entry name. An entry whose record does not fit is named `#<index>`, its
    /// index in the offset table.
    #[error("package entry {name} ends after the end of the package")]
    EntryOutOfBounds { name: String },

    /// A WEM is not a RIFF `WAVE` file with a `fmt ` and a `data` chunk.
    #[error("invalid WEM: {0}")]
    Wem(&'static str),
}
