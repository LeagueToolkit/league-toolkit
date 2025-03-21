use std::{
    fmt::Display,
    io::{self},
};

use super::{read, Tex, Texture};
use byteorder::{ReadBytesExt, LE};

#[derive(Clone, Copy, Hash, Debug, PartialEq, Eq)]
pub enum TextureFileFormat {
    /// https://en.wikipedia.org/wiki/DirectDraw_Surface
    DDS,
    /// League of Legends proprietary format
    TEX,
    Unknown,
}

impl Display for TextureFileFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TextureFileFormat::DDS => "DDS",
            TextureFileFormat::TEX => "TEX",
            TextureFileFormat::Unknown => "unknown",
        })
    }
}

impl TextureFileFormat {
    pub fn from_magic(magic: u32) -> Self {
        match magic {
            0x20534444 => Self::DDS, // "DDS "
            0x00584554 => Self::TEX, // "TEX\0"
            _ => Self::Unknown,
        }
    }

    pub fn read<R: io::Read + io::Seek + ?Sized>(&self, reader: &mut R) -> read::Result<Texture> {
        let magic = reader.read_u32::<LE>()?;
        let from_magic = TextureFileFormat::from_magic(magic);
        if from_magic != *self {
            return Err(read::TextureReadError::UnexpectedTextureFormat(
                *self, from_magic,
            ));
        }
        self.read_no_magic(reader)
    }

    /// Attempts to read a texture of this format - without reading the 4 magic bytes.
    ///
    /// **NOTE**: You **must** make sure the reader does not include the magic bytes!
    pub fn read_no_magic<R: io::Read + io::Seek + ?Sized>(
        &self,
        reader: &mut R,
    ) -> read::Result<Texture> {
        match self {
            TextureFileFormat::DDS => Ok(ddsfile::Dds::read(reader)?.into()),
            TextureFileFormat::TEX => Ok(Tex::from_reader(reader)?.into()),
            _ => Err(read::TextureReadError::UnsupportedTextureFormat(*self)),
        }
    }
}

#[cfg(test)]
mod tests {
    //use image::codecs::png::PngEncoder;
    //use image_dds::image_from_dds;
    //use io::BufWriter;
    //use std::fs;
    //use test_log::test;
    //
    //use super::*;
    //
    //#[test]
    //fn dds() {
    //    let format = TextureFileFormat::DDS;
    //    let mut file = fs::File::open("/home/alan/Downloads/aurora_square_0.aurora.dds").unwrap();
    //    let Texture::Dds(dds) = format.read_no_magic(&mut file).unwrap() else {
    //        unreachable!();
    //    };
    //
    //    let img = image_from_dds(&dds, 0).unwrap();
    //    let out = PngEncoder::new(
    //        std::fs::File::create("./out.png")
    //            .map(BufWriter::new)
    //            .unwrap(),
    //    );
    //    img.write_with_encoder(out).unwrap();
    //
    //    println!("{dds:?}");
    //
    //    panic!();
    //}
}
