use crate::header::V3;

use super::{EntryExt, EntryKind};
use binrw::binrw;
use num_enum::TryFromPrimitive as _;

/// >= 3.4 (post the switch to u24 subchunk_index business)
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy)]
pub struct V3_4 {
    pub path_hash: u64,
    pub data_offset: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,

    #[br(temp)]
    #[bw(calc = (subchunk_count << 4) | (*kind as u8 & 0xF) )]
    kind_subchunk_count: u8,

    #[br(try_calc = EntryKind::try_from_primitive(kind_subchunk_count & 0xF))]
    #[bw(ignore)]
    pub kind: EntryKind,
    #[br(calc = kind_subchunk_count >> 4)]
    #[bw(ignore)]
    pub subchunk_count: u8,

    #[br(map = |x: [u8; 3]| u32::from_le_bytes([x[0], x[1], x[2], 0]))]
    #[bw(try_map = |x: &u32| TryInto::<[u8; 3]>::try_into(&x.to_le_bytes()[0..3]) ) ]
    pub subchunk_index: u32,
    pub checksum: u64,
}

impl V3_4 {
    pub fn from_generic_or_default<E: EntryExt>(value: &E) -> Self {
        Self {
            path_hash: value.path_hash(),
            data_offset: value.data_offset(),
            compressed_size: value.compressed_size(),
            uncompressed_size: value.uncompressed_size(),
            kind: value.kind(),
            subchunk_count: value.subchunk_count().unwrap_or_default(),
            subchunk_index: value.subchunk_index().unwrap_or_default(),
            checksum: value.checksum().unwrap_or_default(),
        }
    }
}

impl V3_4 {
    pub const LATEST_MINOR: u8 = 4;
}

impl EntryExt for V3_4 {
    #[inline(always)]
    fn path_hash(&self) -> u64 {
        self.path_hash
    }

    #[inline(always)]
    fn data_offset(&self) -> u32 {
        self.data_offset
    }

    #[inline(always)]
    fn compressed_size(&self) -> u32 {
        self.compressed_size
    }

    #[inline(always)]
    fn uncompressed_size(&self) -> u32 {
        self.uncompressed_size
    }

    #[inline(always)]
    fn kind(&self) -> EntryKind {
        self.kind
    }

    #[inline(always)]
    fn subchunk_count(&self) -> Option<u8> {
        Some(self.subchunk_count)
    }

    #[inline(always)]
    fn is_duplicate(&self) -> Option<bool> {
        None
    }

    #[inline(always)]
    fn subchunk_index(&self) -> Option<u32> {
        Some(self.subchunk_index)
    }

    #[inline(always)]
    fn checksum(&self) -> Option<u64> {
        Some(self.checksum)
    }
}
