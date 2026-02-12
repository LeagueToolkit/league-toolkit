use crate::{
    property::Kind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty, ReaderExt, WriteProperty, WriterExt},
    Error, PropertyValueEnum,
};
use ltk_io_ext::{ReaderExt as _, WriterExt as _};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Optional {
    pub kind: Kind,
    pub value: Option<Box<PropertyValueEnum>>,
}

impl PropertyValueExt for Optional {
    const KIND: Kind = Kind::Optional;
}

impl PropertyExt for Optional {
    fn size_no_header(&self) -> usize {
        2 + match &self.value {
            Some(inner) => inner.size_no_header(),
            None => 0,
        }
    }
}

impl ReadProperty for Optional {
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
impl WriteProperty for Optional {
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
