use std::io;

use crate::{
    property::Kind,
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty, WriterExt as _},
    PropertyValueEnum,
};
use byteorder::{WriteBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_hash::{BinHash, WriteBytesExt as _};
use ltk_io_ext::{measure, window_at};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Struct {
    pub class_hash: BinHash,
    pub properties: IndexMap<BinHash, PropertyValueEnum>,
}

impl PropertyValueExt for Struct {
    const KIND: Kind = Kind::Struct;
}

impl PropertyExt for Struct {
    fn size_no_header(&self) -> usize {
        match *self.class_hash {
            0 => 4,
            _ => {
                10 + self
                    .properties
                    .values()
                    .map(|p| 5 + p.size_no_header())
                    .sum::<usize>()
            }
        }
    }
}

impl ReadProperty for Struct {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error> {
        owned::read_from(
            reader,
            Kind::Struct,
            Numbering::from_legacy(legacy),
            owned::read_struct,
        )
    }
}
impl WriteProperty for Struct {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy struct writing");
        }

        writer.write_bin_hash::<LE>(self.class_hash)?;

        if *self.class_hash == 0 {
            return Ok(());
        }

        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u16::<LE>(self.properties.len() as _)?;

            for (name_hash, value) in self.properties.iter() {
                writer.write_bin_hash::<LE>(*name_hash)?;
                writer.write_property_kind(value.kind())?;
                value.to_writer(writer)?;
            }

            Ok::<_, io::Error>(())
        })?;

        window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;

        Ok(())
    }
}
