use std::io;

use crate::core::render::texture::CompressedTexture;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unknown extended texture format: {0}")]
    UnknownTextureFormat(u8),
    #[error("Unsupported extended texture format: {0:?}")]
    UnsupportedTextureFormat(ExtendedTextureFormat),
    #[error("Invalid texture flags: {0:#b}")]
    InvalidTextureFlags(u8),
    #[error("IO Error: {0}")]
    IOError(#[from] io::Error),

    #[error("Error reading image data: {0}")]
    DDSError(#[from] image_dds::error::SurfaceError),
}

bitflags::bitflags! {
    struct TextureFlags: u8 {
        const HasMipMaps = 1;
        const Mystery = 2;
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum ExtendedTextureFormat {
    Etc1,
    Etc2Eac,
    Bc1,
    Bc3,
    Bgra8,
}

impl ExtendedTextureFormat {
    pub fn from_u8(format: u8) -> Result<Self, Error> {
        match format {
            1 => Ok(Self::Etc1),
            2 | 3 => Ok(Self::Etc2Eac),
            10 | 11 => Ok(Self::Bc1),
            12 => Ok(Self::Bc3),
            20 => Ok(Self::Bgra8),
            _ => Err(Error::UnknownTextureFormat(format)),
        }
    }

    pub fn try_into_dds_format(self) -> Result<image_dds::ImageFormat, Error> {
        use image_dds::ImageFormat as F;
        match self {
            ExtendedTextureFormat::Bgra8 => Some(F::Bgra8Unorm),
            ExtendedTextureFormat::Etc1 => None,
            ExtendedTextureFormat::Etc2Eac => None,
            ExtendedTextureFormat::Bc1 => Some(F::BC1RgbaUnorm),
            ExtendedTextureFormat::Bc3 => Some(F::BC3RgbaUnorm),
        }
        .ok_or_else(|| Error::UnsupportedTextureFormat(self))
    }
}

#[derive(TryFromPrimitive, IntoPrimitive, Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureFilter {
    None,
    Nearest,
    Linear,
}

#[derive(TryFromPrimitive, IntoPrimitive, Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureAddress {
    Wrap,
    Clamp,
}

use byteorder::{ReadBytesExt, LE};
use num_enum::{IntoPrimitive, TryFromPrimitive, TryFromPrimitiveError};

pub(super) fn read_tex<R: io::Read + io::Seek + ?Sized>(
    reader: &mut R,
) -> Result<CompressedTexture, Error> {
    let (width, height) = (reader.read_u16::<LE>()?, reader.read_u16::<LE>()?);

    let is_extended_format = reader.read_u8(); // maybe..
    let format = ExtendedTextureFormat::from_u8(reader.read_u8()?)?;
    // (0: texture, 1: cubemap, 2: surface, 3: volumetexture)
    let resource_type = reader.read_u8()?; // maybe..

    let flags = reader.read_u8()?;
    let flags = TextureFlags::from_bits(flags).ok_or(Error::InvalidTextureFlags(flags))?;

    let compression_format = format.try_into_dds_format()?;

    let mipmap_count = match flags.contains(TextureFlags::HasMipMaps) {
        true => ((height.max(width) as f32).log2().floor() + 1.0) as u32,
        false => 1,
    };

    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let surface = image_dds::Surface {
        width: 0,
        height: 0,
        depth: 1,
        layers: 1,
        mipmaps: mipmap_count,
        image_format: compression_format,
        data,
    };
    Ok(CompressedTexture::Tex(surface))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use test_log::test;

    use crate::core::render::texture::format::TextureFileFormat;

    use super::*;

    #[test]
    fn test_tex() {
        let format = TextureFileFormat::TEX;
        let mut file =
            fs::File::open("/home/alan/Downloads/aurora_skin02_weapon_tx_cm.chroma_aurora_battlebunny_animasquad_2024.tex").unwrap();
        let tex = format.read_no_magic(&mut file).unwrap();

        log::debug!("{tex:?}");

        panic!();
    }
}
