use crate::{
    property::{values::ContainerItem, Kind, NoMeta},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, ReaderExt, WriteProperty, WriterExt},
    Error, PropertyValueEnum,
};
use ltk_io_ext::{ReaderExt as _, WriterExt as _};

/// At most one value of a declared [`Kind`].
///
/// The kind is declared whether or not a value is present, which is why an empty option still
/// needs one and why [`Optional::item_kind`] always has an answer.
///
/// The format has no nested containers, so a container, option or map cannot be the item. The
/// checked constructors reject those kinds, and [`ContainerItem`] excludes them at compile time.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Clone, PartialEq, Debug)]
pub struct Optional<M = NoMeta> {
    item_kind: Kind,
    // Boxed because `PropertyValueEnum` holds an `Optional`: the format forbids the nesting, but
    // the type does not, so the size has to be broken somewhere.
    value: Option<Box<PropertyValueEnum<M>>>,
    pub meta: M,
}

impl<M: Default> Optional<M> {
    /// An empty option holding items of `item_kind`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `item_kind` is a container kind.
    pub fn empty(item_kind: Kind) -> Result<Self, Error> {
        Self::new(item_kind, None)
    }

    /// An option holding `value`, which must be `item_kind` when it is present.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `item_kind` is a container kind, or
    /// [`Error::MismatchedContainerTypes`] if `value` is not `item_kind`.
    pub fn new(item_kind: Kind, value: Option<PropertyValueEnum<M>>) -> Result<Self, Error> {
        Self::new_with_meta(item_kind, value, M::default())
    }
}

impl<M> Optional<M> {
    /// See [`Optional::new`].
    ///
    /// # Errors
    ///
    /// The same as [`Optional::new`].
    pub fn new_with_meta(
        item_kind: Kind,
        value: Option<PropertyValueEnum<M>>,
        meta: M,
    ) -> Result<Self, Error> {
        if item_kind.is_container() {
            return Err(Error::InvalidNesting(item_kind));
        }
        if let Some(value) = &value {
            if value.kind() != item_kind {
                return Err(Error::MismatchedContainerTypes {
                    expected: item_kind,
                    got: value.kind(),
                });
            }
        }

        Ok(Self {
            item_kind,
            value: value.map(Box::new),
            meta,
        })
    }

    /// The kind of the item this option holds, present or not.
    #[inline(always)]
    #[must_use]
    pub fn item_kind(&self) -> Kind {
        self.item_kind
    }

    /// The contained value, if there is one.
    #[inline(always)]
    #[must_use]
    pub fn value(&self) -> Option<&PropertyValueEnum<M>> {
        self.value.as_deref()
    }

    /// See [`Optional::value`].
    ///
    /// Nothing stops you from writing a value of a different kind, which leaves the option
    /// disagreeing with its own [`Optional::item_kind`] and [`Optional::to_writer`] emitting a
    /// file the game cannot read. Go through [`Optional::set`] to have the kind checked.
    #[inline(always)]
    #[must_use]
    pub fn value_mut(&mut self) -> Option<&mut PropertyValueEnum<M>> {
        self.value.as_deref_mut()
    }

    /// Replaces the contained value, returning the old one.
    ///
    /// # Errors
    ///
    /// [`Error::MismatchedContainerTypes`] if `value` is not [`Optional::item_kind`].
    pub fn set(
        &mut self,
        value: Option<PropertyValueEnum<M>>,
    ) -> Result<Option<PropertyValueEnum<M>>, Error> {
        if let Some(value) = &value {
            if value.kind() != self.item_kind {
                return Err(Error::MismatchedContainerTypes {
                    expected: self.item_kind,
                    got: value.kind(),
                });
            }
        }

        Ok(std::mem::replace(&mut self.value, value.map(Box::new)).map(|v| *v))
    }

    /// The contained value, inserting [`Kind::default_value`] for [`Optional::item_kind`] first if
    /// there is none.
    pub fn value_or_insert_default(&mut self) -> &mut PropertyValueEnum<M>
    where
        M: Default,
    {
        let item_kind = self.item_kind;
        self.value
            .get_or_insert_with(|| Box::new(item_kind.default_value()))
    }

    /// Whether a value is present.
    #[inline(always)]
    #[must_use]
    pub fn is_some(&self) -> bool {
        self.value.is_some()
    }

    /// Whether no value is present.
    #[inline(always)]
    #[must_use]
    pub fn is_none(&self) -> bool {
        self.value.is_none()
    }

    /// The contained value and the metadata, by value.
    #[inline(always)]
    #[must_use]
    pub fn into_parts(self) -> (Option<PropertyValueEnum<M>>, M) {
        (self.value.map(|v| *v), self.meta)
    }

    /// The contained value, by value.
    #[inline(always)]
    #[must_use]
    pub fn into_inner(self) -> Option<PropertyValueEnum<M>> {
        self.into_parts().0
    }

    /// The metadata, by value.
    #[inline(always)]
    #[must_use]
    pub fn into_meta(self) -> M {
        self.into_parts().1
    }

    #[inline(always)]
    #[must_use]
    pub fn no_meta(self) -> Optional<NoMeta> {
        Optional {
            item_kind: self.item_kind,
            value: self.value.map(|v| Box::new(v.no_meta())),
            meta: NoMeta,
        }
    }
}

impl<M: Default> Default for Optional<M> {
    fn default() -> Self {
        Self {
            item_kind: Kind::None,
            value: None,
            meta: M::default(),
        }
    }
}

impl<M: Default, T: ContainerItem + Into<PropertyValueEnum<M>>> From<Option<T>> for Optional<M> {
    fn from(value: Option<T>) -> Self {
        Self {
            item_kind: T::KIND,
            value: value.map(|v| Box::new(v.into())),
            meta: M::default(),
        }
    }
}

impl<M: Default, T: ContainerItem + Into<PropertyValueEnum<M>>> From<T> for Optional<M> {
    fn from(value: T) -> Self {
        Self::from(Some(value))
    }
}

impl<M: Default> TryFrom<PropertyValueEnum<M>> for Optional<M> {
    type Error = Error;

    /// The item kind comes from `value`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `value` is itself a container.
    fn try_from(value: PropertyValueEnum<M>) -> Result<Self, Self::Error> {
        Self::new(value.kind(), Some(value))
    }
}

impl<M> PropertyExt for Optional<M> {
    fn size_no_header(&self) -> usize {
        2 + self
            .value
            .as_ref()
            .map(|v| v.size_no_header())
            .unwrap_or_default()
    }

    type Meta = M;
    fn meta(&self) -> &Self::Meta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut Self::Meta {
        &mut self.meta
    }
}

impl<M> PropertyValueExt for Optional<M> {
    const KIND: Kind = Kind::Optional;
}

impl<M: Default> ReadProperty for Optional<M> {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        let item_kind = reader.read_property_kind(legacy)?;
        if item_kind.is_container() {
            return Err(Error::InvalidNesting(item_kind));
        }

        let value = match reader.read_bool()? {
            true => Some(Box::new(item_kind.read(reader, legacy)?)),
            false => None,
        };

        Ok(Self {
            item_kind,
            value,
            meta: M::default(),
        })
    }
}

impl<M: Clone> WriteProperty for Optional<M> {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        if legacy {
            unimplemented!("legacy optional write")
        }

        writer.write_property_kind(self.item_kind)?;
        writer.write_bool(self.is_some())?;

        match &self.value {
            Some(value) => value.to_writer(writer),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::property::values;

    #[test]
    fn knows_its_item_kind_when_it_is_empty() {
        let empty = Optional::<NoMeta>::empty(Kind::F32).unwrap();
        assert_eq!(empty.item_kind(), Kind::F32);
        assert!(empty.is_none());
        assert_eq!(empty.value(), None);

        let full = Optional::<NoMeta>::from(values::F32::new(1.5));
        assert_eq!(full.item_kind(), Kind::F32);
        assert!(full.is_some());
        assert_eq!(full.value(), Some(&values::F32::new(1.5).into()));
    }

    #[test]
    fn rejects_a_value_that_is_not_its_item_kind() {
        assert!(matches!(
            Optional::<NoMeta>::new(Kind::F32, Some(values::U8::new(1).into())),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::F32,
                got: Kind::U8
            })
        ));

        let mut option = Optional::<NoMeta>::from(values::F32::new(1.5));
        assert!(matches!(
            option.set(Some(values::U8::new(1).into())),
            Err(Error::MismatchedContainerTypes { .. })
        ));

        let old = option.set(Some(values::F32::new(2.5).into())).unwrap();
        assert_eq!(old, Some(values::F32::new(1.5).into()));
        assert_eq!(option.value(), Some(&values::F32::new(2.5).into()));
    }

    #[test]
    fn rejects_a_nested_container() {
        for kind in [
            Kind::Container,
            Kind::UnorderedContainer,
            Kind::Optional,
            Kind::Map,
        ] {
            assert!(matches!(
                Optional::<NoMeta>::empty(kind),
                Err(Error::InvalidNesting(k)) if k == kind
            ));
        }
    }

    #[test]
    fn fills_an_empty_option_with_its_item_kind() {
        let mut option = Optional::<NoMeta>::empty(Kind::Vector2).unwrap();
        assert_eq!(
            *option.value_or_insert_default(),
            Kind::Vector2.default_value()
        );
        assert!(option.is_some());

        // Already filled, so it stays as it was.
        let mut option = Optional::<NoMeta>::from(values::F32::new(1.5));
        assert_eq!(
            *option.value_or_insert_default(),
            values::F32::new(1.5).into()
        );
    }
}
