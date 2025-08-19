use std::io;

use super::EntryKind;

pub trait EntryExt {
    #[must_use]
    fn path_hash(&self) -> u64;
    #[must_use]
    fn data_offset(&self) -> u32;
    #[must_use]
    fn compressed_size(&self) -> u32;
    #[must_use]
    fn uncompressed_size(&self) -> u32;
    #[must_use]
    fn kind(&self) -> EntryKind;

    #[must_use]
    fn subchunk_count(&self) -> Option<u8>;
    #[must_use]
    fn is_duplicate(&self) -> Option<bool>;
    #[must_use]
    fn subchunk_index(&self) -> Option<u32>;

    #[must_use]
    fn checksum(&self) -> Option<u64>;
}

pub trait Decompress {
    fn decompress(&self) -> io::Result<Vec<u8>>;
}
