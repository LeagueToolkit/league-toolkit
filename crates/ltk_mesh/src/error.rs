//! The error type shared by every mesh reader and writer.

/// Something went wrong reading or writing a mesh.
///
/// Reading returns these for files the game itself would reject; writing returns them rather
/// than emit such a file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ParseError {
    /// The file does not start with the format's magic bytes.
    #[error("Invalid file signature")]
    InvalidFileSignature,
    /// The major/minor version pair is not one the format defines.
    #[error("Invalid file version '{0}.{1}'")]
    InvalidFileVersion(u16, u16),
    /// A field holds a value the format does not allow. Names the field, then the value.
    #[error("Invalid '{0}' - got '{1}'")]
    InvalidField(&'static str, String),
    /// The underlying reader or writer failed, including a short read.
    #[error("IO Error - {0}")]
    IOError(#[from] std::io::Error),
    /// A string field is not valid UTF-8.
    #[error("UTF-8 Error - {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    /// A shared reader helper failed.
    #[error(transparent)]
    ReaderError(#[from] ltk_io_ext::ReaderError),
}
