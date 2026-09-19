use crate::{
    property::Kind,
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};

use super::Struct;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Embedded(pub Struct);

impl PropertyValueExt for Embedded {
    const KIND: Kind = Kind::Embedded;
}

impl PropertyExt for Embedded {
    fn size_no_header(&self) -> usize {
        self.0.size_no_header()
    }
}

impl ReadProperty for Embedded {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error> {
        owned::read_from(
            reader,
            Kind::Embedded,
            Numbering::from_legacy(legacy),
            |cur| owned::read_struct(cur).map(Self),
        )
    }
}
impl WriteProperty for Embedded {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        Struct::to_writer(&self.0, writer, legacy)
    }
}
