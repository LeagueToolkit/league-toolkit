use binrw::{BinRead, BinWrite};
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(BinRead, BinWrite, Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
#[brw(little, repr = u8)]
pub enum EntryKind {
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
