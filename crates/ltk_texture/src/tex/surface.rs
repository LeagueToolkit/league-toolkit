use super::super::ToImageError;
use std::borrow::Cow;

/// The uncompressed pixel layout of a decoded surface
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PixelFormat {
    /// 8-bit BGRA, unsigned normalized
    Bgra8Unorm,
    /// 8-bit RGBA, unsigned normalized
    Rgba8Unorm,
    /// 8-bit RG, signed normalized (bytes are `i8` bit patterns)
    Rg8Snorm,
    /// 16-bit RGBA, half-float (channels are little-endian [`half::f16`] bit patterns)
    Rgba16Float,
}

impl PixelFormat {
    pub const fn channel_count(self) -> usize {
        match self {
            PixelFormat::Bgra8Unorm | PixelFormat::Rgba8Unorm | PixelFormat::Rgba16Float => 4,
            PixelFormat::Rg8Snorm => 2,
        }
    }

    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            PixelFormat::Bgra8Unorm | PixelFormat::Rgba8Unorm => 4,
            PixelFormat::Rg8Snorm => 2,
            PixelFormat::Rgba16Float => 8,
        }
    }
}

/// A decoded tex mipmap
///
/// `data` holds tightly-packed, row-major pixels laid out as described by `format`.
/// Decoders emit their *natural* format (e.g. BC5_SNORM decodes to [`PixelFormat::Rg8Snorm`]
/// with the signed data intact) - use [`Self::into_rgba_image`] when you just want something
/// presentable.
pub struct TexSurface<'a> {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub data: Cow<'a, [u8]>,
}

impl TexSurface<'_> {
    /// Reinterpret the raw data as a slice of typed pixels, e.g. `[i8; 2]` for
    /// [`PixelFormat::Rg8Snorm`].
    ///
    /// Returns `None` if `P`'s size/alignment doesn't evenly fit the data.
    pub fn as_pixels<P: bytemuck::AnyBitPattern>(&self) -> Option<&[P]> {
        bytemuck::try_cast_slice(&self.data).ok()
    }

    /// Convert the surface to an [image::RgbaImage]
    ///
    /// This is a *presentation* conversion: signed-normalized channels are remapped from
    /// `[-1, 1]` to `[0, 255]`, float channels are clamped to `[0, 1]` before quantizing
    /// (out-of-range data is lost - use [`Self::as_pixels`] for the real values), and
    /// channels missing from the source format are filled with 0 (alpha with 255).
    pub fn into_rgba_image(self) -> Result<image::RgbaImage, ToImageError> {
        let rgba: Vec<u8> = match self.format {
            PixelFormat::Rgba8Unorm => self.data.into_owned(),
            PixelFormat::Bgra8Unorm => self
                .data
                .chunks_exact(4)
                .flat_map(|pixel| {
                    let [b, g, r, a] = pixel else { unreachable!() };
                    [*r, *g, *b, *a]
                })
                .collect(),
            PixelFormat::Rg8Snorm => self
                .data
                .chunks_exact(2)
                .flat_map(|pixel| {
                    let [r, g] = pixel else { unreachable!() };
                    [
                        snorm8_to_unorm8(*r as i8),
                        snorm8_to_unorm8(*g as i8),
                        0,
                        255,
                    ]
                })
                .collect(),
            PixelFormat::Rgba16Float => self
                .data
                .chunks_exact(2)
                .map(|channel| {
                    let value = half::f16::from_le_bytes([channel[0], channel[1]]).to_f32();
                    (value.clamp(0.0, 1.0) * 255.0).round() as u8
                })
                .collect(),
        };

        image::RgbaImage::from_raw(self.width, self.height, rgba)
            .ok_or(ToImageError::InvalidContainerSize)
    }
}

/// Remap a signed-normalized value from `[-1, 1]` to `[0, 255]`
fn snorm8_to_unorm8(v: i8) -> u8 {
    // -128 is clamped to -127 so the range is symmetric around 0
    let v = v.max(-127) as f32 / 127.0;
    ((v * 0.5 + 0.5) * 255.0).round() as u8
}
