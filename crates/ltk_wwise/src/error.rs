use std::io;

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
}
