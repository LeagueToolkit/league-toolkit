use super::property::BinPropertyKind;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Invalid file signature")]
    InvalidFileSignature,
    #[error("Invalid file version '{0}'")]
    InvalidFileVersion(u32),
    #[error("Invalid '{0}' - got '{1}'")]
    InvalidField(&'static str, String),
    #[error("IO Error - {0}")]
    IOError(#[from] std::io::Error),
    #[error("UTF-8 Error - {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
    #[error(transparent)]
    ReaderError(#[from] crate::util::ReaderError),
    #[error("Invalid property kind - {0}")]
    InvalidPropertyType(#[from] num_enum::TryFromPrimitiveError<BinPropertyKind>),
}
