use binrw::{BinRead, BinWrite};

mod ext;
pub use ext::*;

mod v1;
pub use v1::*;

mod v2;
pub use v2::*;

mod v3;
pub use v3::*;

mod v3_4;
pub use v3_4::*;

mod kind;
pub use kind::*;

mod generic;
pub use generic::*;

#[derive(BinRead, BinWrite)]
#[brw(import {major: u8, minor: u8})]
#[derive(Debug, Clone, Copy)]
pub enum Entry {
    #[br(pre_assert(major == 3 && minor == 4))]
    V3_4(V3_4),
    #[br(pre_assert(major == 3 && minor < 4))]
    V3(V3),
    #[br(pre_assert(major == 2))]
    V2(V2),
    #[br(pre_assert(major == 1))]
    V1(V1),
}
impl Entry {
    pub fn latest_minor(&self) -> u8 {
        match self {
            Entry::V3_4(_) => 4,
            Entry::V3(_) => 3,
            Entry::V2(_) => 0,
            Entry::V1(_) => 0,
        }
    }
}

pub type Latest = V3_4;

impl EntryExt for Entry {
    fn path_hash(&self) -> u64 {
        match self {
            Entry::V3_4(inner) => inner.path_hash(),
            Entry::V3(inner) => inner.path_hash(),
            Entry::V2(inner) => inner.path_hash(),
            Entry::V1(inner) => inner.path_hash(),
        }
    }

    fn data_offset(&self) -> u32 {
        match self {
            Entry::V3_4(inner) => inner.data_offset(),
            Entry::V3(inner) => inner.data_offset(),
            Entry::V2(inner) => inner.data_offset(),
            Entry::V1(inner) => inner.data_offset(),
        }
    }

    fn compressed_size(&self) -> u32 {
        match self {
            Entry::V3_4(inner) => inner.compressed_size(),
            Entry::V3(inner) => inner.compressed_size(),
            Entry::V2(inner) => inner.compressed_size(),
            Entry::V1(inner) => inner.compressed_size(),
        }
    }

    fn uncompressed_size(&self) -> u32 {
        match self {
            Entry::V3_4(inner) => inner.uncompressed_size(),
            Entry::V3(inner) => inner.uncompressed_size(),
            Entry::V2(inner) => inner.uncompressed_size(),
            Entry::V1(inner) => inner.uncompressed_size(),
        }
    }

    fn kind(&self) -> EntryKind {
        match self {
            Entry::V3_4(inner) => inner.kind(),
            Entry::V3(inner) => inner.kind(),
            Entry::V2(inner) => inner.kind(),
            Entry::V1(inner) => inner.kind(),
        }
    }

    fn subchunk_count(&self) -> Option<u8> {
        match self {
            Entry::V3_4(inner) => inner.subchunk_count(),
            Entry::V3(inner) => inner.subchunk_count(),
            Entry::V2(inner) => inner.subchunk_count(),
            Entry::V1(inner) => inner.subchunk_count(),
        }
    }

    fn is_duplicate(&self) -> Option<bool> {
        match self {
            Entry::V3_4(inner) => inner.is_duplicate(),
            Entry::V3(inner) => inner.is_duplicate(),
            Entry::V2(inner) => inner.is_duplicate(),
            Entry::V1(inner) => inner.is_duplicate(),
        }
    }

    fn subchunk_index(&self) -> Option<u32> {
        match self {
            Entry::V3_4(inner) => inner.subchunk_index(),
            Entry::V3(inner) => inner.subchunk_index(),
            Entry::V2(inner) => inner.subchunk_index(),
            Entry::V1(inner) => inner.subchunk_index(),
        }
    }

    fn checksum(&self) -> Option<u64> {
        match self {
            Entry::V3_4(inner) => inner.checksum(),
            Entry::V3(inner) => inner.checksum(),
            Entry::V2(inner) => inner.checksum(),
            Entry::V1(inner) => inner.checksum(),
        }
    }
}
