use std::{hash::Hash, io};

use crate::{
    property::{Kind, NoMeta},
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty, WriterExt},
    Error, PropertyValueEnum, ValueSlot,
};
use byteorder::{WriteBytesExt, LE};
use ltk_io_ext::{measure, window_at};

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

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Map<M = NoMeta> {
    key_kind: Kind,
    value_kind: Kind,
    entries: Vec<(PropertyValueEnum<M>, PropertyValueEnum<M>)>,
    pub meta: M,
}

impl<M> Map<M> {
    #[inline(always)]
    #[must_use]
    pub fn no_meta(self) -> Map<NoMeta> {
        Map {
            key_kind: self.key_kind,
            value_kind: self.value_kind,
            entries: self
                .entries
                .into_iter()
                .map(|(k, v)| (k.no_meta(), v.no_meta()))
                .collect(),
            meta: NoMeta,
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn key_kind(&self) -> Kind {
        self.key_kind
    }

    #[inline(always)]
    #[must_use]
    pub fn value_kind(&self) -> Kind {
        self.value_kind
    }

    #[inline(always)]
    #[must_use]
    pub fn entries(&self) -> &[(PropertyValueEnum<M>, PropertyValueEnum<M>)] {
        &self.entries
    }

    /// A mutable handle on the value of entry `index`, pinned to [`Map::value_kind`].
    ///
    /// There is no plain `&mut` to an entry: writing a value of a different kind would leave the
    /// map disagreeing with its own declared kinds and [`Map::to_writer`] emitting a file the
    /// game cannot read, and a key is not mutable at all, since changing one would reorder the
    /// map behind its own back. Add entries with [`Map::push`].
    #[inline(always)]
    #[must_use]
    pub fn slot(&mut self, index: usize) -> Option<ValueSlot<'_, M>> {
        let value_kind = self.value_kind;
        let (_, value) = self.entries.get_mut(index)?;
        Some(ValueSlot::pinned(value_kind, value))
    }

    #[inline(always)]
    #[must_use]
    pub fn into_entries(self) -> Vec<(PropertyValueEnum<M>, PropertyValueEnum<M>)> {
        self.entries
    }

    #[inline(always)]
    pub fn push(
        &mut self,
        key: PropertyValueEnum<M>,
        value: PropertyValueEnum<M>,
    ) -> Result<(), Error> {
        if self.key_kind != key.kind() {
            return Err(Error::MismatchedContainerTypes {
                expected: self.key_kind,
                got: key.kind(),
            });
        }
        if self.value_kind != value.kind() {
            return Err(Error::MismatchedContainerTypes {
                expected: self.value_kind,
                got: value.kind(),
            });
        }
        self.entries.push((key, value));
        Ok(())
    }
}

impl<M: Default> Map<M> {
    /// An empty map keyed by `key_kind`, holding `value_kind`.
    ///
    /// # Errors
    ///
    /// The same as [`Map::new`].
    pub fn empty(key_kind: Kind, value_kind: Kind) -> Result<Self, Error> {
        Self::new(key_kind, value_kind, Vec::new())
    }

    /// A map of `entries`, whose keys must all be `key_kind` and values all `value_kind`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidKeyType`] if `key_kind` cannot key a map, [`Error::InvalidNesting`] if
    /// `value_kind` is a container kind, or [`Error::MismatchedContainerTypes`] if an entry does
    /// not match the kind it was declared with.
    ///
    /// The first two mirror what [`Map::from_reader`] rejects, so a map that constructs here is
    /// one this crate can read back.
    ///
    /// [`Map::from_reader`]: crate::traits::ReadProperty::from_reader
    pub fn new(
        key_kind: Kind,
        value_kind: Kind,
        entries: Vec<(PropertyValueEnum<M>, PropertyValueEnum<M>)>,
    ) -> Result<Self, Error> {
        if !key_kind.is_valid_map_key() {
            return Err(Error::InvalidKeyType(key_kind));
        }
        if value_kind.is_container() {
            return Err(Error::InvalidNesting(value_kind));
        }
        for (k, v) in &entries {
            if k.kind() != key_kind {
                return Err(Error::MismatchedContainerTypes {
                    expected: key_kind,
                    got: k.kind(),
                });
            }
            if v.kind() != value_kind {
                return Err(Error::MismatchedContainerTypes {
                    expected: value_kind,
                    got: v.kind(),
                });
            }
        }
        Ok(Self {
            key_kind,
            value_kind,
            entries,
            meta: M::default(),
        })
    }
}

impl<M> PropertyValueExt for Map<M> {
    const KIND: Kind = Kind::Map;
}
impl<M> PropertyExt for Map<M> {
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

    type Meta = M;
    fn meta(&self) -> &Self::Meta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut Self::Meta {
        &mut self.meta
    }
}

impl<M: Default> ReadProperty for Map<M> {
    fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        owned::read_from(
            reader,
            Kind::Map,
            Numbering::from_legacy(legacy),
            owned::read_map,
        )
    }
}
impl<M: Clone> WriteProperty for Map<M> {
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
            writer.write_u32::<LE>(self.entries.len() as _)?;

            for (k, v) in self.entries.iter() {
                k.to_writer(writer)?;
                v.to_writer(writer)?;
            }

            Ok::<_, io::Error>(())
        })?;

        window_at(writer, size_pos, |writer| writer.write_u32::<LE>(size as _))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::{values, NoMeta};

    /// The constructors reject exactly what [`Map::from_reader`] rejects, so a map that builds
    /// here is one this crate can read back.
    #[test]
    fn rejects_kinds_its_own_reader_would_refuse() {
        for kind in [
            Kind::Container,
            Kind::UnorderedContainer,
            Kind::Optional,
            Kind::Map,
        ] {
            assert!(matches!(
                Map::<NoMeta>::empty(kind, Kind::I32),
                Err(Error::InvalidKeyType(k)) if k == kind
            ));
            assert!(matches!(
                Map::<NoMeta>::empty(Kind::U32, kind),
                Err(Error::InvalidNesting(k)) if k == kind
            ));
        }

        // All four are fine as a value and none of them as a key. `ObjectLink` is a `u32` hash
        // like `Hash`, so it is excluded by decision rather than by encoding.
        for kind in [
            Kind::Struct,
            Kind::Embedded,
            Kind::BitBool,
            Kind::ObjectLink,
        ] {
            assert!(matches!(
                Map::<NoMeta>::empty(kind, Kind::I32),
                Err(Error::InvalidKeyType(k)) if k == kind
            ));
            assert!(Map::<NoMeta>::empty(Kind::Hash, kind).is_ok());
        }

        assert!(Map::<NoMeta>::empty(Kind::WadChunkLink, Kind::I32).is_ok());
    }

    #[test]
    fn rejects_an_entry_that_does_not_match_its_kinds() {
        assert!(matches!(
            Map::<NoMeta>::new(
                Kind::U32,
                Kind::String,
                vec![(values::I32::new(1).into(), values::String::from("a").into())],
            ),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::U32,
                got: Kind::I32
            })
        ));

        let mut map = Map::<NoMeta>::empty(Kind::U32, Kind::String).unwrap();
        assert!(map
            .push(values::U32::new(1).into(), values::I32::new(2).into())
            .is_err());
        assert!(map
            .push(values::U32::new(1).into(), values::String::from("a").into())
            .is_ok());
        assert_eq!(map.entries().len(), 1);
    }
}
