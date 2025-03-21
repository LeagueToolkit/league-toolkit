use byteorder::{ReadBytesExt, LE};
use std::{io, marker::PhantomData, mem::MaybeUninit};

use image_dds::{Surface, SurfaceRgba8};

use super::{error::ReadError, Compressed, Uncompressed};

#[derive(Debug)]
pub struct Dds<C> {
    file: ddsfile::Dds,
    /// Decompressed surface. **DON'T** access this directly, use [`surface`]
    _surface: MaybeUninit<SurfaceRgba8<Vec<u8>>>,
    _c: PhantomData<C>,
}

impl<C> Dds<C> {
    pub const MAGIC: u32 = u32::from_le_bytes(*b"DDS ");
    pub fn width(&self) -> u32 {
        self.file.get_width()
    }
    pub fn height(&self) -> u32 {
        self.file.get_width()
    }
}

impl Dds<Uncompressed> {
    fn surface(&self) -> &SurfaceRgba8<Vec<u8>> {
        // Safety: this is only uninit when Dds<Compressed>.
        // the only way to get to Dds<Uncompressed>, is via decompress(), which init's _surface
        unsafe { self._surface.assume_init_ref() }
    }
    pub fn to_rgba_image(
        &self,
        mipmap: u32,
    ) -> Result<image::RgbaImage, image_dds::error::CreateImageError> {
        self.surface().to_image(mipmap)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DecodeErr {
    #[error(transparent)]
    DdsErr(#[from] image_dds::error::CreateImageError),
    #[error(transparent)]
    SurfaceErr(#[from] image_dds::error::SurfaceError),
}

impl Dds<Compressed> {
    pub fn decompress(self) -> Result<Dds<Uncompressed>, DecodeErr> {
        let surface = Surface::from_dds(&self.file)?;

        let surface = surface.decode_rgba8()?;

        Ok(Dds {
            file: self.file,
            _surface: MaybeUninit::new(surface),
            _c: PhantomData,
        })
    }
}

impl Dds<Compressed> {
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
    pub fn from_reader_no_magic<R: io::Read + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, ddsfile::Error> {
        ddsfile::Dds::read(reader).map(|file| Self {
            file,
            _surface: MaybeUninit::uninit(),
            _c: PhantomData,
        })
    }
}
