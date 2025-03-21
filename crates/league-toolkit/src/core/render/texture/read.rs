use byteorder::{ReadBytesExt, LE};
use std::io;

use crate::core::render::texture::format::TextureFileFormat;

use super::{tex, ReadError, Texture};

impl Texture {
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(reader: &mut R) -> Result<Self, ReadError> {
        let magic = reader.read_u32::<LE>()?;
        reader.seek(io::SeekFrom::Start(0))?;

        match TextureFileFormat::from_magic(magic) {
            TextureFileFormat::Unknown => Err(ReadError::UnknownTextureFormat(magic)),
            format => format.read_no_magic(reader),
        }
    }
}
