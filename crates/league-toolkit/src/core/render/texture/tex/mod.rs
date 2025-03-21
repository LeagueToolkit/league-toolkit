use bytemuck::{cast_slice, cast_vec};
use byteorder::{ReadBytesExt, LE};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{hint::unreachable_unchecked, io, marker::PhantomData, mem};

mod error;
mod format;

pub use error::*;
pub use format::*;

use super::{format::TextureFileFormat, Compressed, ReadError, ToImageError, Uncompressed};

#[derive(Debug)]
pub struct Tex<C> {
    pub width: u16,
    pub height: u16,
    pub format: Format,
    pub resource_type: u8,
    pub flags: TextureFlags,
    data: Vec<C>,
}

#[derive(thiserror::Error, Debug)]
pub enum DecodeErr {
    #[error("Could not decode ETC1: {0}")]
    Etc1(&'static str),
    #[error("Could not decode ETC2/EAC: {0}")]
    Etc2Eac(&'static str),
    #[error("Could not decode BC3: {0}")]
    Bc3(&'static str),
    #[error("Could not decode BC1: {0}")]
    Bc1(&'static str),
}

impl<C> Tex<C> {
    pub const MAGIC: u32 = u32::from_le_bytes(*b"TEX\0");
    pub fn mipmap_count(&self) -> u32 {
        match self.flags.contains(TextureFlags::HasMipMaps) {
            true => ((self.height.max(self.width) as f32).log2().floor() + 1.0) as u32,
            false => 1,
        }
    }
}

impl Tex<Uncompressed> {
    pub fn to_rgba_image(self) -> Result<image::RgbaImage, ToImageError> {
        image::RgbaImage::from_raw(
            self.width.into(),
            self.height.into(),
            self.data
                .into_iter()
                .flat_map(|pixel| {
                    let [b, g, r, a] = pixel.to_le_bytes();
                    [r, g, b, a]
                })
                .collect(),
        )
        .ok_or(ToImageError::InvalidContainerSize)
    }
}

impl Tex<Compressed> {
    pub fn decompress(self) -> Result<Tex<Uncompressed>, DecodeErr> {
        let data = match matches!(self.format, Format::Bgra8) {
            true => cast_vec(self.data),
            false => {
                let mut data = vec![0; usize::from(self.width) * usize::from(self.height)];
                let (w, h) = (self.width.into(), self.height.into());
                let i = &self.data;
                let o = &mut data;
                match self.format {
                    Format::Etc1 => {
                        texture2ddecoder::decode_etc1(i, w, h, o).map_err(DecodeErr::Etc1)
                    }
                    Format::Bc1 => texture2ddecoder::decode_bc1(i, w, h, o).map_err(DecodeErr::Bc1),
                    Format::Bc3 => texture2ddecoder::decode_bc3(i, w, h, o).map_err(DecodeErr::Bc3),
                    Format::Etc2Eac => {
                        texture2ddecoder::decode_etc2_rgba8(i, w, h, o).map_err(DecodeErr::Etc2Eac)
                    }
                    // Safety: the outer match ensures we can't reach this arm
                    Format::Bgra8 => unsafe { unreachable_unchecked() },
                }?;
                data
            }
        };

        Ok(Tex {
            width: self.width,
            height: self.height,
            format: self.format,
            resource_type: self.resource_type,
            flags: self.flags,
            data,
        })
    }

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
    pub fn from_reader_no_magic<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let (width, height) = (reader.read_u16::<LE>()?, reader.read_u16::<LE>()?);

        let _is_extended_format = reader.read_u8(); // maybe..
        let format = Format::from_u8(reader.read_u8()?)?;
        // (0: texture, 1: cubemap, 2: surface, 3: volumetexture)
        let resource_type = reader.read_u8()?; // maybe..

        let flags = reader.read_u8()?;
        let flags = TextureFlags::from_bits(flags).ok_or(Error::InvalidTextureFlags(flags))?;

        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        Ok(Self {
            width,
            height,
            format,
            flags,
            resource_type,
            data,
        })
    }
}

bitflags::bitflags! {
    #[derive(Debug)]
    pub struct TextureFlags: u8 {
        const HasMipMaps = 1;
        const Mystery = 2;
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

//#[cfg(test)]
//mod tests {
//    use super::*;
//    use image::{buffer::ConvertBuffer as _, codecs::png::PngEncoder};
//    use io::BufWriter;
//    use std::fs;
//    use test_log::test;
//
//    #[test]
//    fn test_tex() {
//        let mut file = fs::File::open("/home/alan/Downloads/srbackground.tex").unwrap();
//        let tex = Tex::from_reader(&mut file).unwrap();
//
//        let tex = tex.decompress().unwrap();
//        let img = tex.to_rgba_image().unwrap();
//
//        let out = PngEncoder::new(
//            std::fs::File::create("./out.png")
//                .map(BufWriter::new)
//                .unwrap(),
//        );
//        img.write_with_encoder(out).unwrap();
//    }
//}
