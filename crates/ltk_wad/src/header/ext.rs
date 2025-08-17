pub trait HeaderExt {
    #[must_use]
    fn toc_offset(&self) -> u16;
    #[must_use]
    fn entry_size(&self) -> u16;
    #[must_use]
    fn entry_count(&self) -> u32;

    /// The checksum of the wad (only present from versions >= 2.x)
    #[must_use]
    #[inline(always)]
    fn checksum(&self) -> Option<u64> {
        None
    }

    /// The signature of the wad (only present from versions >= 3.x)
    #[must_use]
    #[inline(always)]
    fn signature(&self) -> Option<&[u8; 256]> {
        None
    }
}
