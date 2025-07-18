use std::fmt;

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum WadChunkCompression {
    /// Uncompressed
    None = 0,
    /// GZip compressed data
    GZip = 1,
    /// Satellite compressed
    Satellite = 2,
    /// zstd compressed
    Zstd = 3,
    /// zstd compressed data, with some uncompressed data before it
    ZstdMulti = 4,
}

impl fmt::Display for WadChunkCompression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            WadChunkCompression::None => "None",
            WadChunkCompression::GZip => "GZip",
            WadChunkCompression::Satellite => "Satellite",
            WadChunkCompression::Zstd => "Zstd",
            WadChunkCompression::ZstdMulti => "ZstdMulti",
        })
    }
}
