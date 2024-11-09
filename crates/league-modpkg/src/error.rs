use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModpkgError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("IO error: {0}")]
    IoExtError(#[from] io_ext::ReaderError),

    #[error("Invalid modpkg compression type: {0}")]
    InvalidCompressionType(u8),
    #[error("Invalid modpkg license type: {0}")]
    InvalidLicenseType(u8),
    #[error("Invalid modpkg magic: {0}")]
    InvalidMagic(u64),
    #[error("Invalid modpkg version: {0}")]
    InvalidVersion(u32),
    #[error("Duplicate chunk: {0}")]
    DuplicateChunk(u64),
}
