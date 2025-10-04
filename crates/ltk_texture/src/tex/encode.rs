use super::Format;

#[cfg(feature = "intel-tex")]
use intel_tex_2::{bc1, bc3, RgbaSurface};

/// Options for encoding textures
#[derive(Debug, Clone)]
pub struct EncodeOptions {
    /// Texture format to encode to
    pub format: Format,
    /// Whether to generate mipmaps
    pub generate_mipmaps: bool,
    /// Filter type to use for mipmap generation
    pub mipmap_filter: MipmapFilter,
}

/// Filter types for mipmap generation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MipmapFilter {
    /// Nearest neighbor (fastest, lowest quality)
    Nearest,
    /// Triangle/Bilinear (good balance of speed and quality)
    Triangle,
    /// Cubic/Bicubic (slower, higher quality)
    CatmullRom,
    /// Lanczos3 (slowest, highest quality)
    Lanczos3,
}

impl Default for MipmapFilter {
    fn default() -> Self {
        Self::Triangle
    }
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
    pub fn new(format: Format) -> Self {
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
        Self::new(Format::Bc3)
    }
}

/// Encode an RGBA8 image into the specified format
///
/// # Example
/// ```no_run
/// use ltk_texture::tex::{encode_rgba, Format};
///
/// let width = 256;
/// let height = 256;
/// let rgba_data: Vec<u8> = vec![0; (width * height * 4) as usize];
///
/// // Encode to BC3 format
/// let compressed = encode_rgba(width, height, &rgba_data, Format::Bc3).unwrap();
/// ```
pub fn encode_rgba(
    width: u32,
    height: u32,
    rgba_data: &[u8],
    format: Format,
) -> Result<Vec<u8>, EncodeError> {
    match format {
        Format::Bc1 => encode_bc1(width, height, rgba_data),
        Format::Bc3 => encode_bc3(width, height, rgba_data),
        Format::Bgra8 => encode_bgra8(rgba_data),
        _ => Err(EncodeError::UnsupportedFormat(format)),
    }
}

/// Encode an RGBA8 image with mipmaps into the specified format
///
/// Mipmaps are stored from smallest to largest (1x1, 2x2, 4x4, ... up to full size).
///
/// # Example
/// ```no_run
/// use ltk_texture::tex::{encode_rgba_with_mipmaps, Format, MipmapFilter};
/// use image::RgbaImage;
///
/// let img = RgbaImage::new(256, 256);
/// let (data, mip_count) = encode_rgba_with_mipmaps(&img, Format::Bc3, MipmapFilter::Triangle).unwrap();
/// ```
pub fn encode_rgba_with_mipmaps(
    img: &image::RgbaImage,
    format: Format,
    filter: MipmapFilter,
) -> Result<(Vec<u8>, u32), EncodeError> {
    let (width, height) = img.dimensions();

    // Calculate mipmap count
    let mip_count = ((height.max(width) as f32).log2().floor() + 1.0) as u32;

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
                filter.to_image_filter(),
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
        let encoded = encode_rgba(w, h, rgba_data, format)?;
        encoded_data.extend_from_slice(&encoded);
    }

    Ok((encoded_data, mip_count))
}

/// Encode RGBA8 data to BC1 format
#[cfg(feature = "intel-tex")]
fn encode_bc1(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let surface = RgbaSurface {
        data: rgba_data,
        width,
        height,
        stride: 4 * width,
    };
    Ok(bc1::compress_blocks(&surface))
}

#[cfg(not(feature = "intel-tex"))]
fn encode_bc1(_width: u32, _height: u32, _rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    Err(EncodeError::UnsupportedFormat(Format::Bc1))
}

/// Encode RGBA8 data to BC3 format
#[cfg(feature = "intel-tex")]
fn encode_bc3(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    let surface = RgbaSurface {
        data: rgba_data,
        width,
        height,
        stride: 4 * width,
    };
    Ok(bc3::compress_blocks(&surface))
}

#[cfg(not(feature = "intel-tex"))]
fn encode_bc3(_width: u32, _height: u32, _rgba_data: &[u8]) -> Result<Vec<u8>, EncodeError> {
    Err(EncodeError::UnsupportedFormat(Format::Bc3))
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
}
