use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LuaManifestError {
    #[error("invalid magic code (expected {expected:?}, got {actual:?})")]
    InvalidMagic { expected: [u8; 4], actual: [u8; 4] },

    #[error("read error - {0}")]
    ReaderError(#[from] ltk_io_ext::ReaderError),

    #[error("io error")]
    IoError(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, LuaManifestError>;
