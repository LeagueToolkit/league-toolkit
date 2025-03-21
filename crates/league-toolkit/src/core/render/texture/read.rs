use byteorder::{ReadBytesExt, LE};
use image_dds::image_from_dds;
use std::io;

use crate::core::render::texture::format::TextureFileFormat;

use super::{tex, Texture};
use thiserror::Error;

#[derive(Error, Debug)]
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
    #[error("Error decoding DDS file: {0}")]
    DdsDecodeError(#[from] image_dds::error::CreateImageError),
    #[error("Error reading TEX file: {0}")]
    TexError(#[from] tex::Error),
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

    pub fn to_rgba_image(&self, mipmap: u32) -> Result<image::RgbaImage> {
        Ok(match self {
            Self::Dds(dds) => image_from_dds(dds, mipmap)?,
            Self::Tex(tex) => unimplemented!(),
        })
    }
}
