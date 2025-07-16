use std::io;

use super::property::{value::*, BinPropertyKind};
use byteorder::{ReadBytesExt, WriteBytesExt};
use enum_dispatch::enum_dispatch;

const HEADER_SIZE: usize = 5;

/// General methods for property values
#[enum_dispatch]
pub trait PropertyValue {
    /// Get the size of the property value, including the kind header if specified
    fn size(&self, include_header: bool) -> usize {
        self.size_no_header()
            + match include_header {
                true => HEADER_SIZE,
                false => 0,
            }
    }
    fn size_no_header(&self) -> usize;
}

/// Methods for reading properties
pub trait ReadProperty: Sized {
    fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::Error>;
}

/// Extension trait for reading property kinds
pub trait ReaderExt: io::Read {
    /// Reads a u8 as a property kind
    fn read_property_kind(
        &mut self,
        legacy: bool,
    ) -> Result<BinPropertyKind, crate::core::meta::Error> {
        BinPropertyKind::unpack(self.read_u8()?, legacy)
    }
}

impl<R: io::Read + ?Sized> ReaderExt for R {}

/// Methods for writing properties
pub trait WriteProperty: Sized {
    fn to_writer<R: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), io::Error>;
}

/// Extension trait for writing property kinds
pub trait WriterExt: io::Write {
    fn write_property_kind(&mut self, kind: BinPropertyKind) -> Result<(), io::Error> {
        self.write_u8(kind.into())
    }
}

impl<R: io::Write + ?Sized> WriterExt for R {}
