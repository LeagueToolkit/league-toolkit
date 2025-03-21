use byteorder::{ReadBytesExt, LE};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{io, marker::PhantomData};

mod error;
mod format;

pub use error::*;
pub use format::*;

use super::Compressed;

#[derive(Debug)]
pub struct Tex<C> {
    pub width: u16,
    pub height: u16,
    pub format: Format,
    pub resource_type: u8,
    pub flags: TextureFlags,
    data: Vec<u8>,
    _c: PhantomData<C>,
}

impl<C> Tex<C> {
    pub fn mipmap_count(&self) -> u32 {
        match self.flags.contains(TextureFlags::HasMipMaps) {
            true => ((self.height.max(self.width) as f32).log2().floor() + 1.0) as u32,
            false => 1,
        }
    }
}

impl Tex<Compressed> {
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, Error> {
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
            _c: PhantomData,
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

#[cfg(test)]
mod tests {
    //use std::fs;
    //use test_log::test;
    //use crate::core::render::texture::format::TextureFileFormat;
    //use super::*;
    //
    //#[test]
    //fn test_tex() {
    //    let format = TextureFileFormat::TEX;
    //    let mut file =
    //    fs::File::open("/home/alan/Downloads/aurora_skin02_weapon_tx_cm.chroma_aurora_battlebunny_animasquad_2024.tex").unwrap();
    //    let tex = format.read_no_magic(&mut file).unwrap();
    //
    //    log::debug!("{tex:?}");
    //
    //    panic!();
    //}
}
