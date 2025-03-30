//! League extended texture (.tex) & DDS texture handling

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
pub enum Texture {
    Dds(Dds),
    Tex(Tex),
}
pub struct TexSurface<'a> {
    width: u32,
    height: u32,
    data: TexSurfaceData<'a>,
}
pub enum TexSurfaceData<'a> {
    Bgra8Slice(&'a [u8]),
    Bgra8Owned(Vec<u32>),
}
pub enum Surface<'a> {
    Tex(TexSurface<'a>),
    DdsRgba8(image_dds::SurfaceRgba8<Vec<u8>>),
}

impl TexSurface<'_> {
    pub fn into_rgba_image(self) -> Result<image::RgbaImage, ToImageError> {
        image::RgbaImage::from_raw(
            self.width,
            self.height,
            match self.data {
                TexSurfaceData::Bgra8Slice(data) => data
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel else {
                            unreachable!();
                        };
                        [r, g, b, a]
                    })
                    .copied()
                    .collect(),
                TexSurfaceData::Bgra8Owned(vec) => vec
                    .into_iter()
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel.to_le_bytes();
                        [r, g, b, a]
                    })
                    .collect(),
            },
        )
        .ok_or(ToImageError::InvalidContainerSize)
    }
}
impl Surface<'_> {
    pub fn into_rgba_image(self) -> Result<image::RgbaImage, ToImageError> {
        match self {
            Surface::Tex(tex) => tex.into_rgba_image(),
            Surface::DdsRgba8(surface_rgba8) => Ok(surface_rgba8.into_image()?),
        }
    }
}

impl Texture {
    pub fn decode_mipmap(&self, mipmap: u32) -> Result<Surface<'_>, DecompressError> {
        Ok(match self {
            Self::Dds(dds) => Surface::DdsRgba8(dds.decode_mipmap(mipmap)?),
            Self::Tex(tex) => Surface::Tex(tex.decode_mipmap(mipmap)?),
        })
    }
}

impl Texture {
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
    #[inline]
    #[must_use]
    pub fn mip_count(&self) -> u32 {
        match self {
            Texture::Dds(dds) => dds.mip_count(),
            Texture::Tex(tex) => tex.mip_count,
        }
    }
}

impl From<Tex> for Texture {
    fn from(value: Tex) -> Self {
        Self::Tex(value)
    }
}
impl From<Dds> for Texture {
    fn from(value: Dds) -> Self {
        Self::Dds(value)
    }
}
