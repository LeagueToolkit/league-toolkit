use std::{io, marker::PhantomData};

use super::Compressed;

#[derive(Debug)]
pub struct Dds<C> {
    file: ddsfile::Dds,
    _c: PhantomData<C>,
}

impl<C> Dds<C> {
    pub fn width(&self) -> u32 {
        self.file.get_width()
    }
    pub fn height(&self) -> u32 {
        self.file.get_width()
    }

    pub fn to_rgba_image(
        &self,
        mipmap: u32,
    ) -> Result<image::RgbaImage, image_dds::error::CreateImageError> {
        image_dds::image_from_dds(&self.file, mipmap)
    }
}

impl Dds<Compressed> {
    pub fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
    ) -> Result<Self, ddsfile::Error> {
        ddsfile::Dds::read(reader).map(|file| Self {
            file,
            _c: PhantomData,
        })
    }
}
