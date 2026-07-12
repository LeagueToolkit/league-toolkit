use super::Format;
use std::io;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown TEX format: {0}")]
    UnknownTextureFormat(u8),
    #[error("Unsupported TEX format: {0:?}")]
    UnsupportedTextureFormat(Format),
    #[error("Unknown TEX resource type: {0}")]
    UnknownResourceType(u8),
    #[error("Invalid TEX flags: {0:#b}")]
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

#[derive(thiserror::Error, Debug)]
pub enum DecodeErr {
    #[error("Could not decode {0:?}: {1}")]
    Decode(Format, &'static str),
    #[error("Could not decode: {0}")]
    ImageDds(#[from] image_dds::error::SurfaceError),
    #[error(
        "Mip level {level} (bytes {start}..{end}) is out of bounds of the texture data ({len} bytes)"
    )]
    MipOutOfBounds {
        level: u32,
        start: usize,
        end: usize,
        len: usize,
    },
    #[error("Slice {slice} is out of bounds of the texture depth ({depth} slices)")]
    SliceOutOfBounds { slice: u32, depth: usize },
}
