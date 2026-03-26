#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported inibin version: {0}")]
    UnsupportedVersion(u8),

    #[error("string data length mismatch: header says {expected}, actual is {actual}")]
    StringDataLengthMismatch { expected: u16, actual: u16 },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = core::result::Result<T, Error>;
