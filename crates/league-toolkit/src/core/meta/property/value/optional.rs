use crate::core::meta::{
    property::BinPropertyKind,
    traits::{PropertyValue, ReadProperty, WriteProperty},
    ParseError,
};

// I am not a fan of the double tagging, but fixing it would be a whole ordeal (and might not be
// possible with static dispatch?).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct OptionalValue(pub BinPropertyKind, pub Option<Box<PropertyValueEnum>>);

impl PropertyValue for OptionalValue {
    fn size_no_header(&self) -> usize {
        2 + match &self.1 {
            Some(inner) => inner.size_no_header(),
            None => 0,
        }
    }
}

use super::{
    super::super::traits::{ReaderExt as _, WriterExt as _},
    PropertyValueEnum,
};
use crate::util::{ReaderExt as _, WriterExt as _};
impl ReadProperty for OptionalValue {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, ParseError> {
        let kind = reader.read_property_kind(legacy)?;
        if kind.is_container() {
            return Err(ParseError::InvalidNesting(kind));
        }

        let is_some = reader.read_bool()?;

        Ok(Self(
            kind,
            match is_some {
                true => Some(kind.read(reader, legacy)?.into()),
                false => None,
            },
        ))
    }
}
impl WriteProperty for OptionalValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        writer.write_property_kind(self.0)?;
        writer.write_bool(self.1.is_some())?;
        if let Some(value) = &self.1 {
            value.to_writer(writer)?;
        }

        Ok(())
    }
}
