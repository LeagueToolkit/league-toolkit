use super::{EntryExt, EntryKind};
use binrw::binrw;
use num_enum::TryFromPrimitive as _;

#[binrw]
#[derive(Debug, Clone, Copy)]
pub struct V3 {
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

    #[br(map = |x: u8| x == 1)]
    #[bw(map = |x: &bool| u8::from(*x))]
    pub is_duplicate: bool,
    pub subchunk_index: u16,
    pub checksum: u64,
}

impl EntryExt for V3 {
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
        Some(self.subchunk_index.into())
    }

    #[inline(always)]
    fn checksum(&self) -> Option<u64> {
        Some(self.checksum)
    }
}
