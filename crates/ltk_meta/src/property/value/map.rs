use std::{collections::HashMap, hash::Hash, io};

use crate::{
    property::BinPropertyKind,
    traits::{PropertyValue, ReadProperty, ReaderExt, WriteProperty, WriterExt},
    Error,
};
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use ltk_io_ext::{measure, window_at};

use super::PropertyValueEnum;

// FIXME (alan): do with hash here what we do with Eq
impl Hash for PropertyValueEnum {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PropertyValueEnum::None(i) => i.hash(state),
            PropertyValueEnum::Bool(i) => i.hash(state),
            PropertyValueEnum::I8(i) => i.hash(state),
            PropertyValueEnum::U8(i) => i.hash(state),
            PropertyValueEnum::I16(i) => i.hash(state),
            PropertyValueEnum::U16(i) => i.hash(state),
            PropertyValueEnum::I32(i) => i.hash(state),
            PropertyValueEnum::U32(i) => i.hash(state),
            PropertyValueEnum::I64(i) => i.hash(state),
            PropertyValueEnum::U64(i) => i.hash(state),
            PropertyValueEnum::BitBool(i) => i.hash(state),
            _ => std::mem::discriminant(self).hash(state),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct PropertyValueUnsafeEq(pub PropertyValueEnum);
impl Eq for PropertyValueUnsafeEq {}

impl PropertyValue for PropertyValueUnsafeEq {
    fn size_no_header(&self) -> usize {
        self.0.size_no_header()
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct MapValue {
    pub key_kind: BinPropertyKind,
    pub value_kind: BinPropertyKind,
    pub entries: HashMap<PropertyValueUnsafeEq, PropertyValueEnum>,
}

impl PropertyValue for MapValue {
    fn size_no_header(&self) -> usize {
        1 + 1
            + 4
            + 4
            + self
                .entries
                .iter()
                .map(|(k, v)| k.size_no_header() + v.size_no_header())
                .sum::<usize>()
    }
}
impl ReadProperty for MapValue {
    fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        let key_kind = reader.read_property_kind(legacy)?;
        if !key_kind.is_primitive() {
            return Err(Error::InvalidKeyType(key_kind));
        }
        let value_kind = reader.read_property_kind(legacy)?;
        if value_kind.is_container() {
            return Err(Error::InvalidNesting(value_kind));
        }
        let size = reader.read_u32::<LE>()?;
        let (real_size, value) = measure(reader, |reader| {
            let len = reader.read_u32::<LE>()? as _;
            let mut entries = HashMap::with_capacity(len);
            for _ in 0..len {
                entries.insert(
                    key_kind.read(reader, legacy).map(PropertyValueUnsafeEq)?,
                    value_kind.read(reader, legacy)?,
                );
            }
            Ok::<_, Error>(Self {
                key_kind,
                value_kind,
                entries,
            })
        })?;

        if size as u64 != real_size {
            return Err(Error::InvalidSize(size as _, real_size));
        }
        Ok(value)
    }
}
impl WriteProperty for MapValue {
    fn to_writer<R: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), io::Error> {
        if legacy {
            unimplemented!("legacy map writing")
        }

        // FIXME: enforce key/value type restrictions at the type level (or if not possible,
        // assertions at MapValue::new level)
        writer.write_property_kind(self.key_kind)?;
        writer.write_property_kind(self.value_kind)?;

        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u16::<LE>(self.entries.len() as _)?;

            for (k, v) in self.entries.iter() {
                k.0.to_writer(writer)?;
                v.to_writer(writer)?;
            }

            Ok::<_, io::Error>(())
        })?;

        window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;

        Ok(())
    }
}
