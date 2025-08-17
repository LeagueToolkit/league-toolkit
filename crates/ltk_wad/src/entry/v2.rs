use super::{EntryExt, EntryKind};
use binrw::binrw;

#[binrw]
#[derive(Debug, Clone, Copy)]
pub struct V2 {
    pub path_hash: u64,
    pub data_offset: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,

    pub compression: EntryKind,
    pub subchunk_count: u8,
    #[br(map = |x: u8| x == 1)]
    #[bw(map = |x: &bool| u8::from(*x))]
    pub is_duplicate: bool,
    pub subchunk_index: u16,
}

impl EntryExt for V2 {
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
        self.compression
    }

    #[inline(always)]
    fn subchunk_count(&self) -> Option<u8> {
        None
    }

    #[inline(always)]
    fn is_duplicate(&self) -> Option<bool> {
        None
    }

    #[inline(always)]
    fn subchunk_index(&self) -> Option<u32> {
        None
    }

    #[inline(always)]
    fn checksum(&self) -> Option<u64> {
        None
    }
}
