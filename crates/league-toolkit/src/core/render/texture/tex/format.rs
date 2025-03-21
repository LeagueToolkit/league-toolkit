use super::Error;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
#[repr(u8)]
pub enum Format {
    Etc1,
    Etc2Eac,
    Bc1,
    Bc3,
    /// Uncompressed BGRA8
    Bgra8,
}

impl Format {
    pub fn from_u8(format: u8) -> Result<Self, Error> {
        match format {
            1 => Ok(Self::Etc1),
            2 | 3 => Ok(Self::Etc2Eac),
            10 | 11 => Ok(Self::Bc1),
            12 => Ok(Self::Bc3),
            20 => Ok(Self::Bgra8),
            _ => Err(Error::UnknownTextureFormat(format)),
        }
    }

    pub fn try_into_dds_format(self) -> Result<image_dds::ImageFormat, Error> {
        use image_dds::ImageFormat as F;
        match self {
            Format::Bgra8 => Some(F::Bgra8Unorm),
            Format::Etc1 => None,
            Format::Etc2Eac => None,
            Format::Bc1 => Some(F::BC1RgbaUnorm),
            Format::Bc3 => Some(F::BC3RgbaUnorm),
        }
        .ok_or(Error::UnsupportedTextureFormat(self))
    }
}
