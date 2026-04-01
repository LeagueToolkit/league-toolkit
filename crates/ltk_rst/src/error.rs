use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RstError {
    #[error("invalid magic code (expected [0x52, 0x53, 0x54], got {actual:?})")]
    InvalidMagic { actual: [u8; 3] },

    #[error("unsupported RST version: {version:#04x}")]
    UnsupportedVersion { version: u8 },

    #[error("io error")]
    IoError(#[from] io::Error),
}
