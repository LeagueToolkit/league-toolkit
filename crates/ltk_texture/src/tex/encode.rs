use super::Format;

#[cfg(any(feature = "intel-tex", test))]
use std::borrow::Cow;

#[cfg(feature = "intel-tex")]
use intel_tex_2::{bc7, RgbaSurface};

/// Pad RGBA data out to the 4x4 block grid by replicating edge texels.
#[cfg(any(feature = "intel-tex", test))]
fn pad_to_block_grid(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Cow<'_, [u8]>) {
    let padded_w = width.next_multiple_of(4);
    let padded_h = height.next_multiple_of(4);
    if (padded_w == width && padded_h == height) || width == 0 || height == 0 {
        return (width, height, Cow::Borrowed(rgba));
    }

    let (w, pw, ph) = (width as usize, padded_w as usize, padded_h as usize);
    let mut padded = Vec::with_capacity(pw * ph * 4);
    for y in 0..ph {
        let row = &rgba[y.min(height as usize - 1) * w * 4..][..w * 4];
        padded.extend_from_slice(row);
        for _ in w..pw {
            padded.extend_from_slice(&row[(w - 1) * 4..]);
        }
    }
    (padded_w, padded_h, Cow::Owned(padded))
}

/// Texture format to encode to, along with any format-specific options
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EncodeFormat {
    Bc1 {
        /// Weigh colour by alpha during the cluster fit. Off by default; enabling
        /// it can significantly improve perceived quality for textures rendered with
        /// alpha blending, at the cost of color accuracy in transparent regions.
        weigh_colour_by_alpha: bool,
    },
    Bc3 {
        /// Weigh colour by alpha during the cluster fit - see [`EncodeFormat::Bc1`]
        weigh_colour_by_alpha: bool,
    },
    Bc7,
    Bgra8,
    Rgba16Float,
    Rgba32Float,
}

impl From<EncodeFormat> for Format {
    fn from(format: EncodeFormat) -> Self {
        match format {
            EncodeFormat::Bc1 { .. } => Self::Bc1,
            EncodeFormat::Bc3 { .. } => Self::Bc3,
            EncodeFormat::Bc7 => Self::Bc7,
            EncodeFormat::Bgra8 => Self::Bgra8,
            EncodeFormat::Rgba16Float => Self::Rgba16Float,
            EncodeFormat::Rgba32Float => Self::Rgba32Float,
        }
    }
}

impl TryFrom<Format> for EncodeFormat {
    type Error = EncodeError;

    /// Convert a raw tex format into its encode counterpart with default options,
    /// failing with [`EncodeError::UnsupportedFormat`] if the format cannot be encoded
    fn try_from(format: Format) -> Result<Self, EncodeError> {
        Ok(match format {
            Format::Bc1 => Self::Bc1 {
                weigh_colour_by_alpha: false,
            },
            Format::Bc3 => Self::Bc3 {
                weigh_colour_by_alpha: false,
            },
            Format::Bc7 => Self::Bc7,
            Format::Bgra8 => Self::Bgra8,
            Format::Rgba16Float => Self::Rgba16Float,
            Format::Rgba32Float => Self::Rgba32Float,
            format => return Err(EncodeError::UnsupportedFormat(format)),
        })
    }
}

/// Options for encoding textures
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// Texture format to encode to, with any format-specific options
    pub format: EncodeFormat,
    /// Whether to generate mipmaps
    pub generate_mipmaps: bool,
    /// Filter type to use for mipmap generation
    pub mipmap_filter: MipmapFilter,
}

/// Filter types for mipmap generation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MipmapFilter {
    /// Nearest neighbor (fastest, lowest quality)
    Nearest,
    /// Triangle/Bilinear (good balance of speed and quality)
    #[default]
    Triangle,
    /// Cubic/Bicubic (slower, higher quality)
    CatmullRom,
    /// Lanczos3 (slowest, highest quality)
    Lanczos3,
}

impl MipmapFilter {
    fn to_image_filter(self) -> image::imageops::FilterType {
        match self {
            Self::Nearest => image::imageops::FilterType::Nearest,
            Self::Triangle => image::imageops::FilterType::Triangle,
            Self::CatmullRom => image::imageops::FilterType::CatmullRom,
            Self::Lanczos3 => image::imageops::FilterType::Lanczos3,
        }
    }
}

impl EncodeOptions {
    /// Create new options with the specified format and no mipmaps
    pub fn new(format: EncodeFormat) -> Self {
        Self {
            format,
            generate_mipmaps: false,
            mipmap_filter: MipmapFilter::default(),
        }
    }

    /// Enable mipmap generation with default filter. Uses Triangle by default.
    pub fn with_mipmaps(mut self) -> Self {
        self.generate_mipmaps = true;
        self
    }

    /// Set the mipmap filter type
    pub fn with_mipmap_filter(mut self, filter: MipmapFilter) -> Self {
        self.mipmap_filter = filter;
        self
    }
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self::new(EncodeFormat::Bc3 {
            weigh_colour_by_alpha: false,
        })
    }
}

/// Encode an RGBA8 image into the format specified by `options`
///
/// Dimensions don't need to be multiples of 4 - the encoder will pad the image to a 4x4 block grid by replicating edge texels.
///
/// # Example
/// ```no_run
/// use ltk_texture::tex::{encode_rgba, EncodeFormat, EncodeOptions};
///
/// let width = 256;
/// let height = 256;
/// let rgba_data: Vec<u8> = vec![0; (width * height * 4) as usize];
///
/// // Encode to BC3 format
/// let format = EncodeFormat::Bc3 { weigh_colour_by_alpha: false };
/// let compressed =
///     encode_rgba(width, height, &rgba_data, &EncodeOptions::new(format)).unwrap();
/// ```
pub fn encode_rgba(
    width: u32,
    height: u32,
    rgba_data: &[u8],
    options: &EncodeOptions,
) -> Result<Vec<u8>, EncodeError> {
    match options.format {
        EncodeFormat::Bc1 {
            weigh_colour_by_alpha,
        } => encode_texpresso(
            texpresso::Format::Bc1,
            width,
            height,
            rgba_data,
            weigh_colour_by_alpha,
        ),
        EncodeFormat::Bc3 {
            weigh_colour_by_alpha,
        } => encode_texpresso(
            texpresso::Format::Bc3,
            width,
            height,
            rgba_data,
            weigh_colour_by_alpha,
        ),
        EncodeFormat::Bc7 => encode_bc7(width, height, rgba_data),
        EncodeFormat::Bgra8 => encode_bgra8(rgba_data),
        EncodeFormat::Rgba16Float => encode_rgba16_float(rgba_data),
        EncodeFormat::Rgba32Float => encode_rgba32_float(rgba_data),
    }
}

/// Encode RGBA8 data to BC1/BC3 via texpresso's cluster fit
///
/// texpresso handles non-multiple-of-4 dimensions natively by masking out-of-image texels
/// in partial edge blocks from the endpoint fit entirely.
fn encode_texpresso(
    format: texpresso::Format,
    width: u32,
    height: u32,
    rgba_data: &[u8],
    weigh_colour_by_alpha: bool,
) -> Result<Vec<u8>, EncodeError> {
    let (w, h) = (width as usize, height as usize);
    if rgba_data.len() != w * h * 4 {
        return Err(EncodeError::InvalidPixelData);
    }

    let mut out = vec![0u8; format.compressed_size(w, h)];
    format.compress(
        rgba_data,
        w,
        h,
        texpresso::Params {
            algorithm: texpresso::Algorithm::ClusterFit,
            weigh_colour_by_alpha,
            ..Default::default()
        },
        &mut out,
    );

    Ok(out)
}

/// Encode an RGBA8 image with mipmaps into the specified format
///
/// Mipmaps are stored from smallest to largest (1x1, 2x2, 4x4, ... up to full size).
///
/// # Example
/// ```no_run
/// use ltk_texture::tex::{encode_rgba_with_mipmaps, EncodeFormat, EncodeOptions};
/// use image::RgbaImage;
///
/// let img = RgbaImage::new(256, 256);
/// let format = EncodeFormat::Bc3 { weigh_colour_by_alpha: false };
/// let (data, mip_count) =
///     encode_rgba_with_mipmaps(&img, &EncodeOptions::new(format)).unwrap();
/// ```
pub fn encode_rgba_with_mipmaps(
    img: &image::RgbaImage,
    options: &EncodeOptions,
) -> Result<(Vec<u8>, u32), EncodeError> {
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(EncodeError::ZeroSizedImage);
    }

    let mip_count = height.max(width).ilog2() + 1;

    // Generate all mip levels (from full size down to 1x1)
    let mut mip_levels = Vec::new();
    let mut current_img = img.clone();

    for level in 0..mip_count {
        let mip_width = (width >> level).max(1);
        let mip_height = (height >> level).max(1);

        // Resize if not the base level
        if level > 0 {
            current_img = image::imageops::resize(
                &current_img,
                mip_width,
                mip_height,
                options.mipmap_filter.to_image_filter(),
            );
        }

        mip_levels.push(current_img.clone());
    }

    // Encode mipmaps from smallest to largest (reverse order)
    // League .tex format stores: mip[n-1] (1x1), mip[n-2] (2x2), ..., mip[0] (full size)
    let mut encoded_data = Vec::new();

    for img in mip_levels.iter().rev() {
        let (w, h) = img.dimensions();
        let rgba_data = img.as_raw();
        let encoded = encode_rgba(w, h, rgba_data, options)?;
        encoded_data.extend_from_slice(&encoded);
    }

    Ok((encoded_data, mip_count))
}

/// Encode RGBA8 data to BC7 format
#[cfg(feature = "intel-tex")]
fn encode_bc7(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let expected_len = width as usize * height as usize * 4;
    if rgba_data.len() != expected_len {
        return Err(EncodeError::InvalidPixelData);
    }

    let (width, height, rgba) = pad_to_block_grid(width, height, rgba_data);
    let surface = RgbaSurface {
        data: &rgba,
        width,
        height,
        stride: 4 * width,
    };
    Ok(bc7::compress_blocks(&bc7::alpha_basic_settings(), &surface))
}

#[cfg(not(feature = "intel-tex"))]
fn encode_bc7(_width: u32, _height: u32, _rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    Err(EncodeError::UnsupportedFormat(Format::Bc7))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pads_partial_blocks_with_replicated_edges() {
        // 2x1 image [A, B] -> 4x4 where every row is [A, B, B, B]
        let a = [1, 2, 3, 4];
        let b = [5, 6, 7, 8];
        let pixels = [a, b].concat();
        let (w, h, padded) = pad_to_block_grid(2, 1, &pixels);
        assert_eq!((w, h), (4, 4));
        let expected_row = [a, b, b, b].concat();
        for row in padded.chunks_exact(4 * 4) {
            assert_eq!(row, expected_row);
        }

        // block-aligned data is passed through without copying
        let rgba = vec![0u8; 8 * 4 * 4];
        let (w, h, padded) = pad_to_block_grid(8, 4, &rgba);
        assert_eq!((w, h), (8, 4));
        assert!(matches!(padded, Cow::Borrowed(_)));
    }
}

/// Convert RGBA8 to RGBA16 half-float (uncompressed)
fn encode_rgba16_float(rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if !rgba_data.len().is_multiple_of(4) {
        return Err(EncodeError::InvalidPixelData);
    }

    Ok(rgba_data
        .iter()
        .flat_map(|&channel| half::f16::from_f32(channel as f32 / 255.0).to_le_bytes())
        .collect())
}

/// Convert RGBA8 to RGBA32 float (uncompressed)
fn encode_rgba32_float(rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    if !rgba_data.len().is_multiple_of(4) {
        return Err(EncodeError::InvalidPixelData);
    }

    Ok(rgba_data
        .iter()
        .flat_map(|&channel| (channel as f32 / 255.0).to_le_bytes())
        .collect())
}

/// Convert RGBA8 to BGRA8 (uncompressed)
fn encode_bgra8(rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let mut bgra = Vec::with_capacity(rgba_data.len());

    for pixel in rgba_data.chunks_exact(4) {
        let [r, g, b, a] = pixel else {
            return Err(EncodeError::InvalidPixelData);
        };
        bgra.extend_from_slice(&[*b, *g, *r, *a]);
    }

    Ok(bgra)
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error("Unsupported format for encoding: {0:?}")]
    UnsupportedFormat(Format),
    #[error("Invalid pixel data")]
    InvalidPixelData,
    #[error("Cannot encode a zero-sized image")]
    ZeroSizedImage,
}
