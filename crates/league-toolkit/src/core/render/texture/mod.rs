pub mod dds;
pub mod format;
mod read;
pub mod tex;

pub use dds::Dds;
pub use tex::Tex;

pub struct Compressed;
pub struct Decompressed;

#[derive(Debug)]
pub enum Texture<C = Compressed> {
    Dds(Dds<C>),
    Tex(Tex<C>),
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
