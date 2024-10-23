use std::{collections::HashMap, io};

use crate::util::measure;

use super::{BinProperty, ParseError};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
pub struct BinTreeObject {
    pub path_hash: u32,
    pub class_hash: u32,
    pub properties: HashMap<u32, BinProperty>,
}

impl BinTreeObject {
    pub fn from_reader<R: io::Read + io::Seek>(
        reader: &mut R,
        class_hash: u32,
        legacy: bool,
    ) -> Result<Self, ParseError> {
        use byteorder::{ReadBytesExt as _, LE};

        let size = reader.read_u32::<LE>()?;
        let (real_size, value) = measure(reader, |reader| {
            let path_hash = reader.read_u32::<LE>()?;

            let prop_count = reader.read_u16::<LE>()? as usize;
            let mut properties = HashMap::with_capacity(prop_count);
            for _ in 0..prop_count {
                let prop = BinProperty::from_reader(reader, legacy)?;
                properties.insert(prop.name_hash, prop);
            }

            Ok::<_, ParseError>(Self {
                path_hash,
                class_hash,
                properties,
            })
        })?;

        if size as u64 != real_size {
            return Err(ParseError::InvalidSize(size as _, real_size));
        }
        Ok(value)
    }
}
