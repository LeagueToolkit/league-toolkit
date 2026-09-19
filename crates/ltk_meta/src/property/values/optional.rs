use crate::{
    property::{values::ContainerItem, Kind, ValueSlot},
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty, WriterExt},
    Error, PropertyValueEnum,
};
use ltk_io_ext::WriterExt as _;

/// At most one value of a declared [`Kind`].
///
/// The kind is declared whether or not a value is present, which is why an empty option still
/// needs one and why [`Optional::item_kind`] always has an answer.
///
/// The format has no nested containers, so a container, option or map cannot be the item. The
/// checked constructors reject those kinds, and [`ContainerItem`] excludes them at compile time.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct Optional {
    item_kind: Kind,
    // Boxed because `PropertyValueEnum` holds an `Optional`: the format forbids the nesting, but
    // the type does not, so the size has to be broken somewhere.
    value: Option<Box<PropertyValueEnum>>,
}

impl Optional {
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
    pub fn new(item_kind: Kind, value: Option<PropertyValueEnum>) -> Result<Self, Error> {
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
    pub fn value(&self) -> Option<&PropertyValueEnum> {
        self.value.as_deref()
    }

    /// A mutable handle on the contained value, pinned to [`Optional::item_kind`].
    ///
    /// There is no plain `&mut` to the value, because writing one of a different kind would
    /// leave the option disagreeing with its own item kind and [`Optional::to_writer`] emitting a
    /// file the game cannot read. [`ValueSlot`] edits in place freely and checks the kind only on
    /// a whole-value replace.
    #[inline(always)]
    #[must_use]
    pub fn slot(&mut self) -> Option<ValueSlot<'_>> {
        let item_kind = self.item_kind;
        Some(ValueSlot::pinned(item_kind, self.value.as_deref_mut()?))
    }

    /// Replaces the contained value, returning the old one.
    ///
    /// # Errors
    ///
    /// [`Error::MismatchedContainerTypes`] if `value` is not [`Optional::item_kind`].
    pub fn set(
        &mut self,
        value: Option<PropertyValueEnum>,
    ) -> Result<Option<PropertyValueEnum>, Error> {
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

    /// See [`Optional::slot`], inserting [`Kind::default_value`] for [`Optional::item_kind`] first
    /// if there is no value.
    pub fn slot_or_insert_default(&mut self) -> ValueSlot<'_> {
        let item_kind = self.item_kind;
        let value = self
            .value
            .get_or_insert_with(|| Box::new(item_kind.default_value()));
        ValueSlot::pinned(item_kind, value)
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

    /// The contained value, by value.
    #[inline(always)]
    #[must_use]
    pub fn into_inner(self) -> Option<PropertyValueEnum> {
        self.value.map(|v| *v)
    }
}

impl Default for Optional {
    fn default() -> Self {
        Self {
            item_kind: Kind::None,
            value: None,
        }
    }
}

impl<T: ContainerItem + Into<PropertyValueEnum>> From<Option<T>> for Optional {
    fn from(value: Option<T>) -> Self {
        Self {
            item_kind: T::KIND,
            value: value.map(|v| Box::new(v.into())),
        }
    }
}

impl<T: ContainerItem + Into<PropertyValueEnum>> From<T> for Optional {
    fn from(value: T) -> Self {
        Self::from(Some(value))
    }
}

impl TryFrom<PropertyValueEnum> for Optional {
    type Error = Error;

    /// The item kind comes from `value`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `value` is itself a container.
    fn try_from(value: PropertyValueEnum) -> Result<Self, Self::Error> {
        Self::new(value.kind(), Some(value))
    }
}

impl PropertyExt for Optional {
    fn size_no_header(&self) -> usize {
        2 + self
            .value
            .as_ref()
            .map(|v| v.size_no_header())
            .unwrap_or_default()
    }
}

impl PropertyValueExt for Optional {
    const KIND: Kind = Kind::Optional;
}

impl ReadProperty for Optional {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        owned::read_from(
            reader,
            Kind::Optional,
            Numbering::from_legacy(legacy),
            owned::read_optional,
        )
    }
}

impl WriteProperty for Optional {
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
        let empty = Optional::empty(Kind::F32).unwrap();
        assert_eq!(empty.item_kind(), Kind::F32);
        assert!(empty.is_none());
        assert_eq!(empty.value(), None);

        let full = Optional::from(values::F32::new(1.5));
        assert_eq!(full.item_kind(), Kind::F32);
        assert!(full.is_some());
        assert_eq!(full.value(), Some(&values::F32::new(1.5).into()));
    }

    #[test]
    fn rejects_a_value_that_is_not_its_item_kind() {
        assert!(matches!(
            Optional::new(Kind::F32, Some(values::U8::new(1).into())),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::F32,
                got: Kind::U8
            })
        ));

        let mut option = Optional::from(values::F32::new(1.5));
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
                Optional::empty(kind),
                Err(Error::InvalidNesting(k)) if k == kind
            ));
        }
    }

    #[test]
    fn fills_an_empty_option_with_its_item_kind() {
        let mut option = Optional::empty(Kind::Vector2).unwrap();
        assert_eq!(
            option.slot_or_insert_default().get(),
            &Kind::Vector2.default_value()
        );
        assert!(option.is_some());

        // Already filled, so it stays as it was.
        let mut option = Optional::from(values::F32::new(1.5));
        assert_eq!(
            option.slot_or_insert_default().get(),
            &values::F32::new(1.5).into()
        );
    }

    /// The value can be edited in place or replaced by its own kind, and by nothing else.
    #[test]
    fn pins_its_item_kind_to_the_slots_it_hands_out() {
        use crate::property::ValueMut;

        assert!(Optional::empty(Kind::F32).unwrap().slot().is_none());

        let mut option = Optional::from(values::F32::new(1.5));
        let mut slot = option.slot().unwrap();
        assert_eq!(slot.pinned_kind(), Some(Kind::F32));

        assert!(matches!(
            slot.set(values::U8::new(1).into()),
            Err(Error::MismatchedContainerTypes { .. })
        ));

        let ValueMut::F32(value) = slot.as_mut() else {
            panic!("the slot holds an f32");
        };
        value.value = 2.5;

        assert_eq!(option.item_kind(), Kind::F32);
        assert_eq!(option.value(), Some(&values::F32::new(2.5).into()));
    }
}
