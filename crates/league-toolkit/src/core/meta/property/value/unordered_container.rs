use crate::core::meta::traits::{PropertyValue, ReadProperty, WriteProperty};

use super::ContainerValue;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct UnorderedContainerValue(pub ContainerValue);

impl PropertyValue for UnorderedContainerValue {
    fn size_no_header(&self) -> usize {
        self.0.size_no_header()
    }
}

impl ReadProperty for UnorderedContainerValue {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        Ok(Self(ContainerValue::from_reader(reader, legacy)?))
    }
}

impl WriteProperty for UnorderedContainerValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        self.0.to_writer(writer, legacy)
    }
}
