use binrw::binrw;

use super::HeaderExt;

#[binrw]
#[brw(import {
    major: u8,
})]
#[br(pre_assert(major == 3))]
#[bw(assert(major == 3))]
#[derive(Debug, Copy, Clone)]
pub struct V3 {
    pub signature: [u8; 256],
    pub checksum: u64,

    /// Number of TOC entries (# of wad chunks)
    pub entry_count: u32,
}
impl V3 {
    pub const TOC_OFFSET: u16 = 272;
    pub const ENTRY_SIZE: u16 = 32;
    pub const MAJOR: u8 = 3;
}
impl HeaderExt for V3 {
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

    #[inline(always)]
    fn signature(&self) -> Option<&[u8; 256]> {
        Some(&self.signature)
    }
}
