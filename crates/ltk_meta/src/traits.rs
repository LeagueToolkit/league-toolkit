use std::io;

use crate::PropertyValueEnum;

use super::property::Kind;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};

const HEADER_SIZE: usize = 5;

/// General methods for property values
pub trait PropertyExt {
    /// Get the size of the property value, including the kind header if specified
    fn size(&self, include_header: bool) -> usize {
        self.size_no_header()
            + match include_header {
                true => HEADER_SIZE,
                false => 0,
            }
    }
    fn size_no_header(&self) -> usize;

    type Meta;
    fn meta(&self) -> &Self::Meta;
    fn meta_mut(&mut self) -> &mut Self::Meta;
}

pub trait PropertyValueExt {
    const KIND: Kind;
}

pub trait PropertyValueDyn: PropertyExt {
    fn kind(&self) -> Kind;
}

impl<T: PropertyValueExt + PropertyExt> PropertyValueDyn for T {
    fn kind(&self) -> Kind {
        Self::KIND
    }
}

/// Methods for reading properties
pub trait ReadProperty: Sized {
    fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error>;
}

/// Extension trait for reading property kinds
pub trait ReaderExt: io::Read {
    /// Reads a u8 as a property kind
    fn read_property_kind(&mut self, legacy: bool) -> Result<Kind, crate::Error> {
        Kind::unpack(self.read_u8()?, legacy)
    }

    fn read_property<M: Default>(
        &mut self,
        legacy: bool,
    ) -> Result<(u32, PropertyValueEnum<M>), crate::Error>
    where
        Self: io::Seek,
    {
        let name_hash = self.read_u32::<LE>()?;
        let kind = self.read_property_kind(legacy)?;

        Ok((
            name_hash,
            PropertyValueEnum::from_reader(self, kind, legacy)?,
        ))
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
    fn write_property_kind(&mut self, kind: Kind) -> Result<(), io::Error> {
        self.write_u8(kind.into())
    }

    fn write_property<M>(
        &mut self,
        name_hash: u32,
        value: &PropertyValueEnum<M>,
    ) -> Result<(), io::Error>
    where
        M: Clone,
        Self: io::Seek,
    {
        self.write_u32::<LE>(name_hash)?;
        self.write_property_kind(value.kind())?;
        value.to_writer(self)?;
        Ok(())
    }
}

impl<R: io::Write + ?Sized> WriterExt for R {}
