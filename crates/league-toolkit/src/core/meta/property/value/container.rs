use crate::core::meta::traits::{PropertyValue as Value, ReadProperty, ReaderExt};

use super::PropertyValueEnum;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainerValue {
    pub items: Vec<PropertyValueEnum>,
}

impl Value for ContainerValue {
    fn size_no_header(&self) -> usize {
        9 + self.items.iter().map(|p| p.size_no_header()).sum::<usize>()
    }
}

impl ReadProperty for ContainerValue {
    fn from_reader<R: std::io::Read>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        use byteorder::{ReadBytesExt as _, LE};

        let item_kind = reader.read_property_kind(legacy)?;

        let _size = reader.read_u32::<LE>()?;
        let prop_count = reader.read_u32::<LE>()?;
        let mut items = Vec::with_capacity(prop_count as _);
        for _ in 0..prop_count {
            let prop = PropertyValueEnum::from_reader(reader, item_kind, legacy)?;
            items.push(prop);
        }

        // TODO (alan): assert size is valid for the data we read

        Ok(Self { items })
    }
}
