use crate::{
    core::meta::{
        property::BinPropertyKind,
        traits::{PropertyValue as Value, ReadProperty, ReaderExt, WriteProperty, WriterExt},
        ParseError,
    },
    util::measure,
};

use super::PropertyValueEnum;
use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq)]
pub struct ContainerValue {
    pub item_kind: BinPropertyKind,
    pub items: Vec<PropertyValueEnum>,
}

impl Value for ContainerValue {
    fn size_no_header(&self) -> usize {
        9 + self.items.iter().map(|p| p.size_no_header()).sum::<usize>()
    }
}

impl ReadProperty for ContainerValue {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, ParseError> {
        let item_kind = reader.read_property_kind(legacy)?;
        if item_kind.is_container() {
            return Err(ParseError::InvalidNesting(item_kind));
        }

        let size = reader.read_u32::<LE>()?;
        let (real_size, items) = measure(reader, |reader| {
            let prop_count = reader.read_u32::<LE>()?;
            let mut items = Vec::with_capacity(prop_count as _);
            for _ in 0..prop_count {
                let prop = PropertyValueEnum::from_reader(reader, item_kind, legacy)?;
                items.push(prop);
            }
            Ok::<_, ParseError>(items)
        })?;

        if size as u64 != real_size {
            return Err(ParseError::InvalidSize(size as _, real_size));
        }

        Ok(Self { item_kind, items })
    }
}

impl WriteProperty for ContainerValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        writer.write_property_kind(self.item_kind)?;
        writer.write_u32::<LE>(0)?;

        Ok(())
    }
}
