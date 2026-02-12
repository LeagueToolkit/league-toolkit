use crate::{
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
    BinPropertyKind,
};

use super::Struct;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Embedded(pub Struct);

impl PropertyValueExt for Embedded {
    const KIND: BinPropertyKind = BinPropertyKind::Embedded;
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
        Struct::from_reader(reader, legacy).map(Self)
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
