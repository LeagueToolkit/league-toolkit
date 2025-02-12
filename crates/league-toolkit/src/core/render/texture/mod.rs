pub mod format;

mod read;

#[derive(Debug)]
pub enum CompressedTexture {
    Dds(ddsfile::Dds),
    Tex(image_dds::Surface<Vec<u8>>),
}

impl CompressedTexture {
    #[inline]
    #[must_use]
    pub fn width(&self) -> u32 {
        match self {
            CompressedTexture::Dds(dds) => dds.get_width(),
            CompressedTexture::Tex(tex) => tex.width,
        }
    }
    #[inline]
    #[must_use]
    pub fn height(&self) -> u32 {
        match self {
            CompressedTexture::Dds(dds) => dds.get_height(),
            CompressedTexture::Tex(tex) => tex.height,
        }
    }
}

#[derive(Debug)]
pub enum UncompressedTexture {
    Dds(ddsfile::Dds),
    Tex(image_dds::SurfaceRgba8<Vec<u8>>),
}
