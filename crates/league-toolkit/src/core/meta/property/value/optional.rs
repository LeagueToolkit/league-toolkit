use crate::core::meta::traits::{PropertyValue, ReadProperty};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct OptionalValue(pub Option<Box<PropertyValueEnum>>);

impl PropertyValue for OptionalValue {
    fn size_no_header(&self) -> usize {
        2 + match &self.0 {
            Some(inner) => inner.size_no_header(),
            None => 0,
        }
    }
}

use super::{super::super::traits::ReaderExt as _, PropertyValueEnum};
use crate::util::ReaderExt as _;
impl ReadProperty for OptionalValue {
    fn from_reader<R: std::io::Read>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        let kind = reader.read_property_kind(legacy)?;
        let is_some = reader.read_bool()?;

        Ok(Self(match is_some {
            true => Some(kind.read(reader, legacy)?.into()),
            false => None,
        }))
    }
}
