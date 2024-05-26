mod r#static;
pub use r#static::*;

mod skinned;
pub use skinned::*;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid file signature")]
    InvalidFileSignature,
    #[error("Invalid file version")]
    InvalidFileVersion,
    #[error("IO Error")]
    ReaderError(#[from] std::io::Error),
    #[error("UTF-8 Error")]
    Utf8Error(#[from] std::str::Utf8Error),
}

pub type Result<T> = core::result::Result<T, ParseError>;
