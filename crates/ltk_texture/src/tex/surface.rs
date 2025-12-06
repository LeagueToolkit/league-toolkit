use half::f16;
use image::{ImageBuffer, Rgba};

use super::super::ToImageError;

/// 16-bit unsigned integer RGBA image buffer
///
/// Used as output format for [TexSurface::into_rgba16_image].
/// For f16 source textures, values are converted from 0.0-1.0 float range to 0-65535 integer range.
pub type Rgba16Image = ImageBuffer<Rgba<u16>, Vec<u16>>;

/// A decoded tex mipmap
pub struct TexSurface<'a> {
    pub width: u32,
    pub height: u32,
    pub data: TexSurfaceData<'a>,
}

/// The data of a tex surface
pub enum TexSurfaceData<'a> {
    Bgra8Slice(&'a [u8]),
    Bgra8Owned(Vec<u32>),
    /// Half-precision float (f16) per channel BGRA data (8 bytes per pixel)
    Bgra16fSlice(&'a [u8]),
}

impl TexSurface<'_> {
    /// Convert the surface to an [image::RgbaImage] (8-bit per channel)
    ///
    /// For 16-bit textures, this will normalize values to 8-bit.
    /// Use [Self::into_rgba16_image] to preserve full precision.
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
                TexSurfaceData::Bgra16fSlice(data) => data
                    .chunks_exact(8)
                    .flat_map(|pixel| {
                        let b = f16::from_le_bytes([pixel[0], pixel[1]]).to_f32();
                        let g = f16::from_le_bytes([pixel[2], pixel[3]]).to_f32();
                        let r = f16::from_le_bytes([pixel[4], pixel[5]]).to_f32();
                        let a = f16::from_le_bytes([pixel[6], pixel[7]]).to_f32();

                        [
                            (r.clamp(0.0, 1.0) * 255.0) as u8,
                            (g.clamp(0.0, 1.0) * 255.0) as u8,
                            (b.clamp(0.0, 1.0) * 255.0) as u8,
                            (a.clamp(0.0, 1.0) * 255.0) as u8,
                        ]
                    })
                    .collect(),
            },
        )
        .ok_or(ToImageError::InvalidContainerSize)
    }

    /// Convert the surface to an [Rgba16Image] (16-bit per channel)
    ///
    /// For 8-bit textures, values are scaled up to 16-bit.
    pub fn into_rgba16_image(self) -> Result<Rgba16Image, ToImageError> {
        Rgba16Image::from_raw(
            self.width,
            self.height,
            match self.data {
                TexSurfaceData::Bgra8Slice(data) => data
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel else {
                            unreachable!();
                        };

                        [
                            *r as u16 * 257,
                            *g as u16 * 257,
                            *b as u16 * 257,
                            *a as u16 * 257,
                        ]
                    })
                    .collect(),
                TexSurfaceData::Bgra8Owned(vec) => vec
                    .into_iter()
                    .flat_map(|pixel| {
                        let [b, g, r, a] = pixel.to_le_bytes();

                        [
                            r as u16 * 257,
                            g as u16 * 257,
                            b as u16 * 257,
                            a as u16 * 257,
                        ]
                    })
                    .collect(),
                TexSurfaceData::Bgra16fSlice(data) => data
                    .chunks_exact(8)
                    .flat_map(|pixel| {
                        let b = f16::from_le_bytes([pixel[0], pixel[1]]).to_f32();
                        let g = f16::from_le_bytes([pixel[2], pixel[3]]).to_f32();
                        let r = f16::from_le_bytes([pixel[4], pixel[5]]).to_f32();
                        let a = f16::from_le_bytes([pixel[6], pixel[7]]).to_f32();

                        [
                            (r.clamp(0.0, 1.0) * 65535.0) as u16,
                            (g.clamp(0.0, 1.0) * 65535.0) as u16,
                            (b.clamp(0.0, 1.0) * 65535.0) as u16,
                            (a.clamp(0.0, 1.0) * 65535.0) as u16,
                        ]
                    })
                    .collect(),
            },
        )
        .ok_or(ToImageError::InvalidContainerSize)
    }
}
