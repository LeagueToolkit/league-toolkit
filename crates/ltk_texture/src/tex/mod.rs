use byteorder::{ReadBytesExt, LE};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{borrow::Cow, io};

mod bc5_snorm;
mod encode;
mod error;
mod format;
mod surface;
mod write;

pub use encode::*;
pub use error::*;
pub use format::*;
pub use surface::*;

use super::ReadError;

/// Signature of `texture2ddecoder`'s surface decode functions
type Texture2dDecodeFn = fn(&[u8], usize, usize, &mut [u32]) -> Result<(), &'static str>;

/// League extended texture file (.tex)
#[derive(Debug)]
pub struct Tex {
    pub width: u16,
    pub height: u16,
    pub format: Format,
    pub resource_type: u8,
    pub flags: TextureFlags,
    pub mip_count: u32,
    data: Vec<u8>,
}

impl Tex {
    pub const MAGIC: u32 = u32::from_le_bytes(*b"TEX\0");

    /// Checks the texture flags for whether the texture contains mipmaps
    pub fn has_mipmaps(&self) -> bool {
        self.flags.contains(TextureFlags::HasMipMaps)
    }

    /// Encode a new Tex from an RGBA image with encoding options
    ///
    /// # Example
    /// ```no_run
    /// use ltk_texture::Tex;
    /// use ltk_texture::tex::{EncodeOptions, Format, MipmapFilter};
    /// use image::RgbaImage;
    ///
    /// let img = RgbaImage::new(256, 256);
    ///
    /// // Without mipmaps
    /// let tex = Tex::encode_rgba_image(&img, EncodeOptions::new(Format::Bc3)).unwrap();
    ///
    /// // With mipmaps
    /// let tex_mips = Tex::encode_rgba_image(
    ///     &img,
    ///     EncodeOptions::new(Format::Bc3).with_mipmaps()
    /// ).unwrap();
    ///
    /// // With mipmaps and custom filter
    /// let tex_lanczos = Tex::encode_rgba_image(
    ///     &img,
    ///     EncodeOptions::new(Format::Bc3)
    ///         .with_mipmaps()
    ///         .with_mipmap_filter(MipmapFilter::Lanczos3)
    /// ).unwrap();
    /// ```
    pub fn encode_rgba_image(
        img: &image::RgbaImage,
        options: EncodeOptions,
    ) -> Result<Self, EncodeError> {
        let (width, height) = img.dimensions();

        let (data, mip_count, flags) = if options.generate_mipmaps {
            let (mip_data, mip_count) =
                encode_rgba_with_mipmaps(img, options.format, options.mipmap_filter)?;
            (mip_data, mip_count, TextureFlags::HasMipMaps)
        } else {
            let rgba_data = img.as_raw();
            let encoded = encode_rgba(width, height, rgba_data, options.format)?;
            (encoded, 1, TextureFlags::empty())
        };

        Ok(Self {
            width: width as u16,
            height: height as u16,
            format: options.format,
            resource_type: 0, // texture
            flags,
            mip_count,
            data,
        })
    }

    /// Encode a new Tex from a DynamicImage with encoding options
    ///
    /// # Example
    /// ```no_run
    /// use ltk_texture::Tex;
    /// use ltk_texture::tex::{EncodeOptions, Format};
    ///
    /// let img = image::open("texture.png").unwrap();
    /// let tex = Tex::encode_dynamic_image(
    ///     img,
    ///     EncodeOptions::new(Format::Bc3).with_mipmaps()
    /// ).unwrap();
    /// ```
    pub fn encode_dynamic_image(
        img: image::DynamicImage,
        options: EncodeOptions,
    ) -> Result<Self, EncodeError> {
        Self::encode_rgba_image(&img.to_rgba8(), options)
    }
}

impl Tex {
    /// Try to decode a single mipmap, where 0 is full resolution, and [Self::mip_count] is the smallest
    /// mip (1x1).
    pub fn decode_mipmap(&self, level: u32) -> Result<TexSurface<'_>, DecodeErr> {
        let level = level.min(self.mip_count - 1);
        let width = self.width as usize;
        let height = self.height as usize;
        let (block_w, block_h) = self.format.block_size();

        let mip_dims = |level: u32| ((width >> level).max(1), (height >> level).max(1));
        let mip_bytes = |dims: (usize, usize)| {
            (dims.0.div_ceil(block_w)) * (dims.1.div_ceil(block_h)) * self.format.bytes_per_block()
        };

        // sum all mips before our one
        // (league sorts mips smallest -> largest so our iterator counts up)
        let off = (level + 1..self.mip_count)
            .map(|level| mip_bytes(mip_dims(level)))
            .sum::<usize>();

        // size of mip
        let (w, h) = mip_dims(level);

        let mip_data =
            self.data
                .get(off..off + mip_bytes((w, h)))
                .ok_or(DecodeErr::MipOutOfBounds {
                    level,
                    start: off,
                    end: off + mip_bytes((w, h)),
                    len: self.data.len(),
                })?;

        // decodes a block-compressed mip to RGBA8 via image_dds
        let decode_image_dds = |image_format| -> Result<Vec<u8>, DecodeErr> {
            let surface = image_dds::Surface {
                width: w as u32,
                height: h as u32,
                depth: 1,
                layers: 1,
                mipmaps: 1,
                image_format,
                data: mip_data,
            };
            Ok(surface.decode_layers_mipmaps_rgba8(0..1, 0..1)?.data)
        };

        // decodes a mip to BGRA8 via texture2ddecoder
        let decode_texture2d = |decode: Texture2dDecodeFn| -> Result<Vec<u8>, DecodeErr> {
            let mut data = vec![0u32; w * h];
            decode(mip_data, w, h, &mut data)
                .map_err(|reason| DecodeErr::Decode(self.format, reason))?;
            // the u32 pixels are BGRA8 packed as little-endian
            Ok(data.into_iter().flat_map(u32::to_le_bytes).collect())
        };

        use image_dds::ImageFormat as IF;
        let (format, data): (PixelFormat, Cow<'_, [u8]>) = match self.format {
            // TODO: test me (this is likely wrong)
            Format::Bgra8 => (PixelFormat::Bgra8Unorm, Cow::Borrowed(mip_data)),
            Format::Bc1 => (
                PixelFormat::Rgba8Unorm,
                decode_image_dds(IF::BC1RgbaUnorm)?.into(),
            ),
            Format::Bc3 => (
                PixelFormat::Rgba8Unorm,
                decode_image_dds(IF::BC3RgbaUnorm)?.into(),
            ),
            Format::Bc7 => (
                PixelFormat::Rgba8Unorm,
                decode_image_dds(IF::BC7RgbaUnormSrgb)?.into(),
            ),
            // image_dds/texture2ddecoder only decode *unsigned* BC5, so we do it ourselves
            Format::Bc5Snorm => (
                PixelFormat::Rg8Snorm,
                bc5_snorm::decode_bc5_snorm(mip_data, w, h).into(),
            ),
            Format::Etc1 => (
                PixelFormat::Bgra8Unorm,
                decode_texture2d(texture2ddecoder::decode_etc1)?.into(),
            ),
            Format::Etc2Eac => (
                PixelFormat::Bgra8Unorm,
                decode_texture2d(texture2ddecoder::decode_etc2_rgba8)?.into(),
            ),
        };

        Ok(TexSurface {
            width: w as _,
            height: h as _,
            format,
            data,
        })
    }

    pub fn from_reader<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, ReadError> {
        let magic = reader.read_u32::<LE>()?; // skip magic
        if magic != Self::MAGIC {
            return Err(ReadError::UnexpectedMagic {
                expected: Self::MAGIC,
                got: magic,
            });
        }

        Ok(Self::from_reader_no_magic(reader)?)
    }
    pub fn from_reader_no_magic<R: io::Read + ?Sized>(reader: &mut R) -> Result<Self, Error> {
        let (width, height) = (reader.read_u16::<LE>()?, reader.read_u16::<LE>()?);

        let _is_extended_format = reader.read_u8(); // maybe..
        let format = Format::from_u8(reader.read_u8()?)?;

        // (0: texture, 1: cubemap, 2: surface, 3: volumetexture)
        let resource_type = reader.read_u8()?; // maybe..

        let flags = reader.read_u8()?;
        let flags = TextureFlags::from_bits(flags).ok_or(Error::InvalidTextureFlags(flags))?;

        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;

        Ok(Self {
            width,
            height,
            format,
            flags,
            resource_type,
            data,
            mip_count: match flags.contains(TextureFlags::HasMipMaps) {
                true => ((height.max(width) as f32).log2().floor() + 1.0) as u32,
                false => 1,
            },
        })
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct TextureFlags: u8 {
        const HasMipMaps = 1;
        const Mystery = 2;
    }
}

#[derive(TryFromPrimitive, IntoPrimitive, Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureFilter {
    None,
    Nearest,
    Linear,
}

#[derive(TryFromPrimitive, IntoPrimitive, Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum TextureAddress {
    Wrap,
    Clamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_new_format_ids() {
        assert_eq!(Format::from_u8(13).unwrap(), Format::Bc7);
        assert_eq!(Format::from_u8(14).unwrap(), Format::Bc5Snorm);
        assert_eq!(Format::Bc7.to_u8(), 13);
        assert_eq!(Format::Bc5Snorm.to_u8(), 14);
    }

    fn tex_file(format: u8, data: &[u8]) -> Vec<u8> {
        let mut file = Vec::new();
        file.extend_from_slice(b"TEX\0");
        file.extend_from_slice(&4u16.to_le_bytes()); // width
        file.extend_from_slice(&4u16.to_le_bytes()); // height
        file.push(0); // is_extended_format
        file.push(format);
        file.push(0); // resource type: texture
        file.push(0); // flags: no mipmaps
        file.extend_from_slice(data);
        file
    }

    #[test]
    fn reads_and_decodes_bc5_snorm_tex() {
        // one BC5 block: red = constant 1.0 (endpoints 127/-127, all indices 0),
        // green = constant -1.0 (endpoints -127/-127)
        let mut block = Vec::new();
        block.extend_from_slice(&[127, 0x81, 0, 0, 0, 0, 0, 0]);
        block.extend_from_slice(&[0x81, 0x81, 0, 0, 0, 0, 0, 0]);
        let file = tex_file(14, &block);

        let tex = Tex::from_reader(&mut file.as_slice()).unwrap();
        assert_eq!(tex.format, Format::Bc5Snorm);
        assert_eq!(tex.mip_count, 1);

        // the decoded surface preserves the signed data
        let surface = tex.decode_mipmap(0).unwrap();
        assert_eq!(surface.format, PixelFormat::Rg8Snorm);
        assert_eq!(surface.as_pixels::<[i8; 2]>().unwrap(), [[127, -127]; 16]);

        // ...while the image conversion remaps to [0, 255]
        let img = surface.into_rgba_image().unwrap();
        assert_eq!(img.dimensions(), (4, 4));
        for pixel in img.pixels() {
            assert_eq!(pixel.0, [255, 0, 0, 255]);
        }
    }

    #[test]
    fn truncated_data_errors_instead_of_panicking() {
        let file = tex_file(13, &[0; 4]); // BC7 4x4 needs a full 16-byte block
        let tex = Tex::from_reader(&mut file.as_slice()).unwrap();
        assert!(matches!(
            tex.decode_mipmap(0),
            Err(DecodeErr::MipOutOfBounds { .. })
        ));
    }
}

//#[cfg(test)]
//mod old_tests {
//    use super::*;
//    use image::codecs::png::PngEncoder;
//    use io::BufWriter;
//    use std::fs;
//    use test_log::test;
//
//    #[test]
//    fn tex() {
//        let mut file = fs::File::open("/home/alan/Downloads/ashe_base_2011_tx_cm.tex").unwrap();
//        let tex = Tex::from_reader(&mut file).unwrap();
//
//        dbg!(tex.format);
//        dbg!(tex.width, tex.height);
//        dbg!(tex.has_mipmaps());
//        dbg!(tex.mip_count);
//        for i in 0..tex.mip_count {
//            let tex = tex.decode_mipmap(i).unwrap();
//            let img = tex.into_rgba_image().unwrap();
//
//            let out = PngEncoder::new(
//                std::fs::File::create(format!("./out_{i}.png"))
//                    .map(BufWriter::new)
//                    .unwrap(),
//            );
//            img.write_with_encoder(out).unwrap();
//        }
//        panic!("success");
//    }
//}
