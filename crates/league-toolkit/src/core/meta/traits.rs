use std::io;

use super::property::{value::*, BinPropertyKind};
use enum_dispatch::enum_dispatch;

const HEADER_SIZE: usize = 5;
#[enum_dispatch]
pub trait PropertyValue {
    fn size(&self, include_header: bool) -> usize {
        self.size_no_header()
            + match include_header {
                true => HEADER_SIZE,
                false => 0,
            }
    }
    fn size_no_header(&self) -> usize;
}

pub trait ReadProperty: Sized {
    fn from_reader<R: io::Read + io::Seek>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError>;
}

use byteorder::ReadBytesExt as _;
pub trait ReaderExt: io::Read {
    /// Reads a u8 as a property kind
    fn read_property_kind(
        &mut self,
        legacy: bool,
    ) -> Result<BinPropertyKind, crate::core::meta::ParseError> {
        BinPropertyKind::unpack(self.read_u8()?, legacy)
    }
}

impl<R: io::Read + ?Sized> ReaderExt for R {}
