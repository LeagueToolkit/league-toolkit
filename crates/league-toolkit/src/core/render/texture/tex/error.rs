use super::Format;
use std::io;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown extended texture format: {0}")]
    UnknownTextureFormat(u8),
    #[error("Unsupported extended texture format: {0:?}")]
    UnsupportedTextureFormat(Format),
    #[error("Invalid texture flags: {0:#b}")]
    InvalidTextureFlags(u8),
    #[error("IO Error: {0}")]
    IOError(#[from] io::Error),

    #[error("Could not make image - invalid dimensions")]
    InvalidDimensions,

    #[error("Error reading image data: {0}")]
    DDSError(#[from] image_dds::error::SurfaceError),
    #[error(transparent)]
    TexError(#[from] super::DecodeErr),
}
