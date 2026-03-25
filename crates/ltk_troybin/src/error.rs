use crate::types::StorageType;

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum TroybinError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Empty troybin data")]
    Empty,

    #[error("Unknown troybin version: {0}")]
    UnknownVersion(u8),

    #[error(
        "Unexpected end of data at offset {offset} (needed {needed} bytes, {available} available)"
    )]
    UnexpectedEof {
        offset: usize,
        needed: usize,
        available: usize,
    },

    #[error("INI parse error at line {line}: {message}")]
    IniParse { line: usize, message: String },

    #[error("Cannot write v1 (old format) entries to binary — convert to v2 storage types first")]
    V1WriteNotSupported,

    #[error("Invalid storage type: {0}")]
    InvalidStorageType(#[from] num_enum::TryFromPrimitiveError<StorageType>),
}
