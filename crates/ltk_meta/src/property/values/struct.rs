use std::io;

use crate::{
    property::{Kind, NoMeta},
    traits::{
        PropertyExt, PropertyValueExt, ReadProperty, ReaderExt, WriteProperty, WriterExt as _,
    },
    Error, PropertyValueEnum,
};
use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use indexmap::IndexMap;
use ltk_io_ext::{measure, window_at};

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Struct<M = NoMeta> {
    pub class_hash: u32,
    pub properties: IndexMap<u32, PropertyValueEnum<M>>,
    pub meta: M,
}

impl<M> Struct<M> {
    #[inline(always)]
    #[must_use]
    pub fn no_meta(self) -> Struct<NoMeta> {
        Struct {
            class_hash: self.class_hash,
            properties: self
                .properties
                .into_iter()
                .map(|(k, v)| (k, v.no_meta()))
                .collect(),
            meta: NoMeta,
        }
    }
}

impl<M> PropertyValueExt for Struct<M> {
    const KIND: Kind = Kind::Struct;
}

impl<M> PropertyExt for Struct<M> {
    fn size_no_header(&self) -> usize {
        match self.class_hash {
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

    type Meta = M;
    fn meta(&self) -> &Self::Meta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut Self::Meta {
        &mut self.meta
    }
}

impl<M: Default> ReadProperty for Struct<M> {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error> {
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
            let mut properties = IndexMap::with_capacity(prop_count as _);
            for _ in 0..prop_count {
                let (name_hash, value) = reader.read_property::<M>(legacy)?;
                properties.insert(name_hash, value);
            }
            Ok::<_, Error>(Self {
                class_hash,
                properties,
                meta: M::default(),
            })
        })?;

        if size as u64 != real_size {
            return Err(crate::Error::InvalidSize(size as _, real_size));
        }

        Ok(value)
    }
}
impl<M: Clone> WriteProperty for Struct<M> {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy struct writing");
        }

        writer.write_u32::<LE>(self.class_hash)?;

        if self.class_hash == 0 {
            return Ok(());
        }

        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u16::<LE>(self.properties.len() as _)?;

            for (name_hash, value) in self.properties.iter() {
                writer.write_u32::<LE>(*name_hash)?;
                writer.write_property_kind(value.kind())?;
                value.to_writer(writer)?;
            }

            Ok::<_, io::Error>(())
        })?;

        window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;

        Ok(())
    }
}
