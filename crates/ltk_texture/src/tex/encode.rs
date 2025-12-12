use super::Format;

#[cfg(feature = "intel-tex")]
use intel_tex_2::{bc1, bc3, RgbaSurface};

#[cfg(any(feature = "intel-tex", test))]
#[inline]
fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[cfg(any(feature = "intel-tex", test))]
#[inline]
fn quantize_to_bits(x: f32, bits: u8) -> f32 {
    debug_assert!((1..=8).contains(&bits));
    let levels = (1u32 << bits) - 1;
    (x * levels as f32).round() / levels as f32
}

/// Dither RGB toward 5/6/5 (BC1/BC3 color endpoint-ish). Alpha is untouched.
///
/// This uses **8×8 Bayer ordered dithering** (deterministic thresholding).
#[cfg(any(feature = "intel-tex", test))]
fn ordered_dither_rgb565_bayer8_in_place(width: u32, height: u32, rgba: &mut [u8]) {
    let w = width as usize;
    let h = height as usize;
    if w == 0 || h == 0 {
        return;
    }
    debug_assert_eq!(rgba.len(), w * h * 4);

    // 8×8 Bayer matrix values in [0, 63]
    // (classic ordered dithering threshold map)
    const BAYER8: [[u8; 8]; 8] = [
        [0, 48, 12, 60, 3, 51, 15, 63],
        [32, 16, 44, 28, 35, 19, 47, 31],
        [8, 56, 4, 52, 11, 59, 7, 55],
        [40, 24, 36, 20, 43, 27, 39, 23],
        [2, 50, 14, 62, 1, 49, 13, 61],
        [34, 18, 46, 30, 33, 17, 45, 29],
        [10, 58, 6, 54, 9, 57, 5, 53],
        [42, 26, 38, 22, 41, 25, 37, 21],
    ];

    // Dither amplitude: half of one quantization step in normalized space.
    // For bits b: step = 1/((2^b)-1), so we apply ±0.5*step.
    let step_r = 1.0 / ((1u32 << 5) - 1) as f32;
    let step_g = 1.0 / ((1u32 << 6) - 1) as f32;
    let step_b = 1.0 / ((1u32 << 5) - 1) as f32;

    for y in 0..h {
        for x in 0..w {
            let idx = (y * w + x) * 4;
            let t = (BAYER8[y & 7][x & 7] as f32 + 0.5) / 64.0 - 0.5; // [-0.5, +0.5)

            let r = rgba[idx] as f32 / 255.0;
            let g = rgba[idx + 1] as f32 / 255.0;
            let b = rgba[idx + 2] as f32 / 255.0;

            let r_d = clamp01(r + t * step_r);
            let g_d = clamp01(g + t * step_g);
            let b_d = clamp01(b + t * step_b);

            let rq = quantize_to_bits(r_d, 5);
            let gq = quantize_to_bits(g_d, 6);
            let bq = quantize_to_bits(b_d, 5);

            rgba[idx] = (rq * 255.0).round().clamp(0.0, 255.0) as u8;
            rgba[idx + 1] = (gq * 255.0).round().clamp(0.0, 255.0) as u8;
            rgba[idx + 2] = (bq * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

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
    let expected_len = width as usize * height as usize * 4;
    if rgba_data.len() != expected_len {
        return Err(EncodeError::InvalidPixelData);
    }

    // Pre-dither toward RGB565 to reduce visible block artifacts in BC1.
    let mut rgba = rgba_data.to_vec();
    ordered_dither_rgb565_bayer8_in_place(width, height, &mut rgba);

    let surface = RgbaSurface {
        data: &rgba,
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
    let expected_len = width as usize * height as usize * 4;
    if rgba_data.len() != expected_len {
        return Err(EncodeError::InvalidPixelData);
    }

    // Pre-dither toward RGB565 to reduce visible block artifacts in BC3 (color endpoints).
    let mut rgba = rgba_data.to_vec();
    ordered_dither_rgb565_bayer8_in_place(width, height, &mut rgba);

    let surface = RgbaSurface {
        data: &rgba,
        width,
        height,
        stride: 4 * width,
    };
    Ok(bc3::compress_blocks(&surface))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed_levels(bits: u8) -> std::collections::HashSet<u8> {
        let levels = (1u32 << bits) - 1;
        (0..=levels)
            .map(|n| {
                ((n as f32 / levels as f32) * 255.0)
                    .round()
                    .clamp(0.0, 255.0) as u8
            })
            .collect()
    }

    #[test]
    fn dither_preserves_alpha_and_quantizes_rgb_to_565_levels() {
        let (w, h) = (8u32, 4u32);
        let mut rgba = vec![0u8; (w * h * 4) as usize];

        // Fill with a gradient-ish pattern and varying alpha
        for y in 0..h as usize {
            for x in 0..w as usize {
                let idx = (y * w as usize + x) * 4;
                rgba[idx] = (x as u8).wrapping_mul(31);
                rgba[idx + 1] = (y as u8).wrapping_mul(47);
                rgba[idx + 2] = ((x + y) as u8).wrapping_mul(19);
                rgba[idx + 3] = (255u8).wrapping_sub((x as u8).wrapping_mul(17));
            }
        }

        let alpha_before: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
        ordered_dither_rgb565_bayer8_in_place(w, h, &mut rgba);
        let alpha_after: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
        assert_eq!(alpha_before, alpha_after);

        let allowed_r = allowed_levels(5);
        let allowed_g = allowed_levels(6);
        let allowed_b = allowed_levels(5);

        for px in rgba.chunks_exact(4) {
            assert!(allowed_r.contains(&px[0]));
            assert!(allowed_g.contains(&px[1]));
            assert!(allowed_b.contains(&px[2]));
        }
    }
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
