use binrw::{BinRead, BinWrite};

mod ext;
pub use ext::*;

mod v1;
pub use v1::*;

mod v2;
pub use v2::*;

mod v3;
pub use v3::*;

#[derive(BinRead, BinWrite)]
#[brw(import (
    major: u8,
))]
#[derive(Debug, Copy, Clone)]
#[allow(clippy::large_enum_variant)] // most wads are v3 now so who cares
pub enum Header {
    V3(#[brw(args {major})] V3),
    V2(#[brw(args {major})] V2),
    V1(#[brw(args {major})] V1),
}
pub type Latest = V3;

impl Header {
    pub fn major(&self) -> u8 {
        match self {
            Header::V1(_) => 1,
            Header::V2(_) => 2,
            Header::V3(_) => 3,
        }
    }
}

impl HeaderExt for Header {
    fn checksum(&self) -> Option<u64> {
        match self {
            Header::V1(inner) => inner.checksum(),
            Header::V2(inner) => inner.checksum(),
            Header::V3(inner) => inner.checksum(),
        }
    }

    fn signature(&self) -> Option<&[u8; 256]> {
        match self {
            Header::V1(inner) => inner.signature(),
            Header::V2(inner) => inner.signature(),
            Header::V3(inner) => inner.signature(),
        }
    }

    fn toc_offset(&self) -> u16 {
        match self {
            Header::V1(inner) => inner.toc_offset(),
            Header::V2(inner) => inner.toc_offset(),
            Header::V3(inner) => inner.toc_offset(),
        }
    }

    fn entry_size(&self) -> u16 {
        match self {
            Header::V1(inner) => inner.entry_size(),
            Header::V2(inner) => inner.entry_size(),
            Header::V3(inner) => inner.entry_size(),
        }
    }

    fn entry_count(&self) -> u32 {
        match self {
            Header::V1(inner) => inner.entry_count(),
            Header::V2(inner) => inner.entry_count(),
            Header::V3(inner) => inner.entry_count(),
        }
    }
}
