use std::collections::HashMap;

use crate::core::meta::{
    traits::{PropertyValue as Value, ReadProperty},
    BinProperty,
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StructValue {
    pub class_hash: u32,
    pub properties: HashMap<u32, BinProperty>,
}

impl Value for StructValue {
    fn size_no_header(&self) -> usize {
        match self.class_hash {
            0 => 4,
            _ => 10 + self.properties.values().map(|p| p.size()).sum::<usize>(),
        }
    }
}

impl ReadProperty for StructValue {
    fn from_reader<R: std::io::Read>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        use crate::util::ReaderExt as _;
        use byteorder::{ReadBytesExt as _, LE};

        let class_hash = reader.read_u32::<LE>()?;
        if class_hash == 0 {
            return Ok(Self {
                class_hash,
                ..Default::default()
            });
        }

        let _size = reader.read_u32::<LE>()?;
        let prop_count = reader.read_u16::<LE>()?;
        let mut properties = HashMap::with_capacity(prop_count as _);
        for _ in 0..prop_count {
            let prop = BinProperty::from_reader(reader, legacy)?;
            properties.insert(prop.name_hash, prop);
        }

        // TODO (alan): assert size is valid for the data we read

        Ok(Self {
            class_hash,
            properties,
        })
    }
}
