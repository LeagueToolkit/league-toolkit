use crate::core::meta::traits::{PropertyValue, ReadProperty};

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
    fn from_reader<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        Ok(Self(ContainerValue::from_reader(reader, legacy)?))
    }
}
