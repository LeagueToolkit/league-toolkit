pub mod dds;
pub mod error;
pub mod format;
mod read;
pub mod tex;

pub use dds::Dds;
pub use error::*;
pub use tex::Tex;

pub type Compressed = u8;
pub type Uncompressed = u32;

#[derive(Debug)]
pub enum Texture<C = Compressed> {
    Dds(Dds<C>),
    Tex(Tex<C>),
}

impl Texture<Compressed> {
    pub fn decompress(self) -> Result<Texture<Uncompressed>, DecompressError> {
        match self {
            Texture::Dds(dds) => Ok(dds.decompress()?.into()),
            Texture::Tex(tex) => Ok(tex.decompress()?.into()),
        }
    }
}

impl Texture<Uncompressed> {
    pub fn to_rgba_image(self, mipmap: u32) -> Result<image::RgbaImage, ToImageError> {
        Ok(match self {
            Self::Dds(dds) => dds.to_rgba_image(mipmap)?,
            Self::Tex(tex) => tex.to_rgba_image()?,
        })
    }
}

impl<C> Texture<C> {
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        match self {
            Texture::Dds(dds) => dds.width(),
            Texture::Tex(tex) => tex.width.into(),
        }
    }
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        match self {
            Texture::Dds(dds) => dds.height(),
            Texture::Tex(tex) => tex.height.into(),
        }
    }
}

impl<C> From<Tex<C>> for Texture<C> {
    fn from(value: Tex<C>) -> Self {
        Self::Tex(value)
    }
}
impl<C> From<Dds<C>> for Texture<C> {
    fn from(value: Dds<C>) -> Self {
        Self::Dds(value)
    }
}
