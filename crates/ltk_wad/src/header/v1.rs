use binrw::binrw;

use super::HeaderExt;

#[binrw]
#[brw(import {
    major: u8,
})]
#[br(pre_assert(major == 1))]
#[bw(assert(major == 1))]
#[derive(Debug, Copy, Clone)]
pub struct V1 {
    #[bw(calc = Self::TOC_OFFSET)]
    #[br(assert(toc_offset == Self::TOC_OFFSET))]
    pub toc_offset: u16,

    /// Size of a single TOC entry (wad chunk)
    #[bw(calc = Self::ENTRY_SIZE)]
    #[br(assert(entry_size == Self::ENTRY_SIZE))]
    pub entry_size: u16,

    /// Number of TOC entries (# of wad chunks)
    pub entry_count: u32,
}

impl V1 {
    pub const TOC_OFFSET: u16 = 12;
    pub const ENTRY_SIZE: u16 = 24;
}

impl HeaderExt for V1 {
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
}
