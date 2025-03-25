use byteorder::{ReadBytesExt, LE};
use std::io;

use image_dds::SurfaceRgba8;

use super::error::ReadError;

#[derive(Debug)]
pub struct Dds {
    file: ddsfile::Dds,
}

impl Dds {
    pub const MAGIC: u32 = u32::from_le_bytes(*b"DDS ");

    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        self.file.get_width()
    }

    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        self.file.get_width()
    }

    #[inline]
    #[must_use]
    pub fn mip_count(&self) -> u32 {
        self.file.get_num_mipmap_levels()
    }
}

impl Dds {
    #[inline]
    pub fn decode_mipmap(&self, mipmap: u32) -> Result<SurfaceRgba8<Vec<u8>>, DecodeErr> {
        Ok(image_dds::Surface::from_dds(&self.file)?
            .decode_layers_mipmaps_rgba8(0..self.file.get_num_array_layers(), mipmap..mipmap + 1)?)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DecodeErr {
    #[error(transparent)]
    DdsErr(#[from] image_dds::error::CreateImageError),
    #[error(transparent)]
    SurfaceErr(#[from] image_dds::error::SurfaceError),
}

impl Dds {
    pub fn from_reader<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, ReadError> {
        let magic = reader.read_u32::<LE>()?; // skip magic
        if magic != Self::MAGIC {
            return Err(ReadError::UnexpectedMagic {
                expected: Self::MAGIC,
                got: magic,
            });
        }
        Ok(Self::from_reader_no_magic(reader)?)
    }

    #[inline]
    pub fn from_reader_no_magic<R: io::Read + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, ddsfile::Error> {
        ddsfile::Dds::read(reader).map(|file| Self { file })
    }
}
