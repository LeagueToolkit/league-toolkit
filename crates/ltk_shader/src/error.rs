use std::io;

use ltk_wad::WadError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("io error")]
    Io(#[from] io::Error),

    #[error("wad error")]
    Wad(#[from] WadError),

    #[error("shader object not found: {path}")]
    ShaderObjectNotFound { path: String },

    #[error("shader bundle not found: {path}")]
    ShaderBundleNotFound { path: String },

    #[error("shader for defines `{defines}` not found in TOC")]
    DefinesNotFound { defines: String },

    #[error("invalid TOC magic: expected {expected:?}, got {actual:?}")]
    InvalidTocMagic { expected: String, actual: String },

    #[error("invalid section magic: expected {expected:?}, got {actual:?}")]
    InvalidSectionMagic { expected: String, actual: String },

    #[error("TOC vector length mismatch: expected {expected}, got {hashes} hashes and {ids} ids")]
    TocLengthMismatch {
        expected: usize,
        hashes: usize,
        ids: usize,
    },
}

pub type Result<T> = std::result::Result<T, ShaderError>;
