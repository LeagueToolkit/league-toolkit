use binrw::{binrw, BinRead, BinWrite};

mod ext;
use derive_more::{Deref, DerefMut};
pub use ext::*;

mod v1;
pub use v1::*;

mod v2;
pub use v2::*;

mod v3;
pub use v3::*;

#[binrw]
#[brw(magic = b"RW", little)]
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct Header {
    #[bw(calc = self.major())]
    major: u8,
    minor: u8,
    #[brw(args(major))]
    #[deref]
    #[deref_mut]
    inner: Headers,
}
impl Header {
    pub fn major(&self) -> u8 {
        self.inner.major()
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }

    pub fn version(&self) -> (u8, u8) {
        (self.major(), self.minor())
    }
}

#[derive(BinRead, BinWrite)]
#[brw(import (
    major: u8,
))]
#[derive(Debug, Copy, Clone)]
#[allow(clippy::large_enum_variant)] // most wads are v3 now so who cares
pub enum Headers {
    V3(#[brw(args {major})] V3),
    V2(#[brw(args {major})] V2),
    V1(#[brw(args {major})] V1),
}
pub type Latest = V3;

impl Headers {
    pub fn major(&self) -> u8 {
        match self {
            Headers::V1(_) => 1,
            Headers::V2(_) => 2,
            Headers::V3(_) => 3,
        }
    }
}

impl HeaderExt for Headers {
    fn checksum(&self) -> Option<u64> {
        match self {
            Headers::V1(inner) => inner.checksum(),
            Headers::V2(inner) => inner.checksum(),
            Headers::V3(inner) => inner.checksum(),
        }
    }

    fn signature(&self) -> Option<&[u8; 256]> {
        match self {
            Headers::V1(inner) => inner.signature(),
            Headers::V2(inner) => inner.signature(),
            Headers::V3(inner) => inner.signature(),
        }
    }

    fn toc_offset(&self) -> u16 {
        match self {
            Headers::V1(inner) => inner.toc_offset(),
            Headers::V2(inner) => inner.toc_offset(),
            Headers::V3(inner) => inner.toc_offset(),
        }
    }

    fn entry_size(&self) -> u16 {
        match self {
            Headers::V1(inner) => inner.entry_size(),
            Headers::V2(inner) => inner.entry_size(),
            Headers::V3(inner) => inner.entry_size(),
        }
    }

    fn entry_count(&self) -> u32 {
        match self {
            Headers::V1(inner) => inner.entry_count(),
            Headers::V2(inner) => inner.entry_count(),
            Headers::V3(inner) => inner.entry_count(),
        }
    }
}
