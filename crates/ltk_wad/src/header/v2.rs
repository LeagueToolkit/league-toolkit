use binrw::binrw;

use super::HeaderExt;

#[binrw]
#[brw(import {
    major: u8,
})]
#[br(pre_assert(major == 2))]
#[bw(assert(major == 2))]
#[derive(Debug, Copy, Clone)]
pub struct V2 {
    #[brw(pad_before = 84)] // skip the unused signature bytes
    pub checksum: u64,

    #[bw(calc = 104)]
    #[br(assert(toc_offset == 104))]
    pub toc_offset: u16,

    /// Size of a single TOC entry (wad chunk)
    #[bw(calc = 24)]
    #[br(assert(entry_size == 24))]
    pub entry_size: u16,

    /// Number of TOC entries (# of wad chunks)
    pub entry_count: u32,
}
impl V2 {
    pub const TOC_OFFSET: u16 = 104;
    pub const ENTRY_SIZE: u16 = 24;
}
impl HeaderExt for V2 {
    #[inline(always)]
    fn toc_offset(&self) -> u16 {
        Self::TOC_OFFSET
    }

    #[inline(always)]
    fn entry_size(&self) -> u16 {
        Self::ENTRY_SIZE
    }

    #[inline(always)]
    fn entry_count(&self) -> u32 {
        self.entry_count
    }

    #[inline(always)]
    fn checksum(&self) -> Option<u64> {
        Some(self.checksum)
    }
}
