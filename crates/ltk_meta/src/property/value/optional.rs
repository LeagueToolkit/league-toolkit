use super::PropertyValueEnum;
use crate::{
    property::BinPropertyKind,
    traits::{PropertyValue, ReadProperty, ReaderExt, WriteProperty, WriterExt},
    Error,
};
use ltk_io_ext::{ReaderExt as _, WriterExt as _};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct OptionalValue {
    pub kind: BinPropertyKind,
    pub value: Option<Box<PropertyValueEnum>>,
}

impl PropertyValue for OptionalValue {
    fn size_no_header(&self) -> usize {
        2 + match &self.value {
            Some(inner) => inner.size_no_header(),
            None => 0,
        }
    }
}

impl ReadProperty for OptionalValue {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        let kind = reader.read_property_kind(legacy)?;
        if kind.is_container() {
            return Err(Error::InvalidNesting(kind));
        }

        let is_some = reader.read_bool()?;

        Ok(Self {
            kind,
            value: match is_some {
                true => Some(kind.read(reader, legacy)?.into()),
                false => None,
            },
        })
    }
}
impl WriteProperty for OptionalValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy optional write")
        }
        writer.write_property_kind(self.kind)?;
        writer.write_bool(self.value.is_some())?;
        if let Some(value) = &self.value {
            value.to_writer(writer)?;
        }

        Ok(())
    }
}
