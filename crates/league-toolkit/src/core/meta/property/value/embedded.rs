use crate::core::meta::traits::{PropertyValue, ReadProperty, WriteProperty};

use super::StructValue;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct EmbeddedValue(pub StructValue);

impl PropertyValue for EmbeddedValue {
    fn size_no_header(&self) -> usize {
        self.0.size_no_header()
    }
}

impl ReadProperty for EmbeddedValue {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        StructValue::from_reader(reader, legacy).map(Self)
    }
}
impl WriteProperty for EmbeddedValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        todo!()
    }
}
