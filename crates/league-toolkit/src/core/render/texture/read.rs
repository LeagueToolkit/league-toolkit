use byteorder::{ReadBytesExt, LE};
use std::io;

use crate::core::render::texture::format::TextureFileFormat;

use super::Texture;

#[derive(thiserror::Error, Debug)]
pub enum TextureReadError {
    #[error("Invalid texture file format! Got magic: '{0:#x}'")]
    UnknownTextureFormat(u32),
    #[error("Unsupported texture file format - '{0}'!")]
    UnsupportedTextureFormat(TextureFileFormat),
    #[error("Unexpected texture file format! expected {0}, got {1}")]
    UnexpectedTextureFormat(TextureFileFormat, TextureFileFormat),

    #[error("IO error: {0}")]
    IOError(#[from] io::Error),
    #[error("Error reading DDS file: {0}")]
    DdsError(#[from] ddsfile::Error),
}

pub type Result<T> = core::result::Result<T, TextureReadError>;

impl Texture {
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self> {
        let magic = reader.read_u32::<LE>()?;
        reader.seek(io::SeekFrom::Start(0))?;

        match TextureFileFormat::from_magic(magic) {
            TextureFileFormat::Unknown => Err(TextureReadError::UnknownTextureFormat(magic)),
            format => format.read_no_magic(reader),
        }
    }
}
