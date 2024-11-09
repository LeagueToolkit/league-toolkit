use std::{collections::HashMap, io};

use crate::core::meta::{
    traits::{PropertyValue as Value, ReadProperty, WriteProperty},
    BinProperty, ParseError,
};
use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use io_ext::{measure, window};

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
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        let class_hash = reader.read_u32::<LE>()?;
        if class_hash == 0 {
            return Ok(Self {
                class_hash,
                ..Default::default()
            });
        }

        let size = reader.read_u32::<LE>()?;

        let (real_size, value) = measure(reader, |reader| {
            let prop_count = reader.read_u16::<LE>()?;
            let mut properties = HashMap::with_capacity(prop_count as _);
            for _ in 0..prop_count {
                let prop = BinProperty::from_reader(reader, legacy)?;
                properties.insert(prop.name_hash, prop);
            }
            Ok::<_, ParseError>(Self {
                class_hash,
                properties,
            })
        })?;

        if size as u64 != real_size {
            return Err(crate::core::meta::ParseError::InvalidSize(
                size as _, real_size,
            ));
        }

        Ok(value)
    }
}
impl WriteProperty for StructValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy struct writing");
        }

        writer.write_u32::<LE>(self.class_hash)?;

        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u16::<LE>(self.properties.len() as _)?;

            for prop in self.properties.values() {
                prop.to_writer(writer)?;
            }

            Ok::<_, io::Error>(())
        })?;

        window(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;

        Ok(())
    }
}
