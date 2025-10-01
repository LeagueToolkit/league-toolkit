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

    pub fn to_u8(&self) -> u8 {
        match self {
            Format::Etc1 => 1,
            Format::Etc2Eac => 2,
            Format::Bc1 => 10,
            Format::Bc3 => 12,
            Format::Bgra8 => 20,
        }
    }

    /// Get the block size of the format
    pub fn block_size(&self) -> (usize, usize) {
        match self {
            Format::Bgra8 => (1, 1),
            _ => (4, 4),
        }
    }

    /// Get the bytes per block of the format
    pub fn bytes_per_block(&self) -> usize {
        match self {
            Format::Etc1 => 8,
            Format::Etc2Eac => 16,
            Format::Bc1 => 8,
            Format::Bc3 => 16,
            Format::Bgra8 => 4,
        }
    }
}
