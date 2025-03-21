pub mod format;
mod read;
pub mod tex;

pub use tex::Tex;

#[derive(Debug)]
pub enum Texture {
    Dds(ddsfile::Dds),
    Tex(Tex),
}

impl Texture {
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        match self {
            Texture::Dds(dds) => dds.get_width(),
            Texture::Tex(tex) => tex.width.into(),
        }
    }
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        match self {
            Texture::Dds(dds) => dds.get_height(),
            Texture::Tex(tex) => tex.height.into(),
        }
    }
}

impl From<Tex> for Texture {
    fn from(value: Tex) -> Self {
        Self::Tex(value)
    }
}
impl From<ddsfile::Dds> for Texture {
    fn from(value: ddsfile::Dds) -> Self {
        Self::Dds(value)
    }
}
