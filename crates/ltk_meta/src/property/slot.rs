use crate::{
    property::{FromValue, Kind, NoMeta, ValueMut},
    Error, PropertyValueEnum,
};

/// A mutable handle on one value, carrying whatever kind its holder pins it to.
///
/// Where a value sits decides whether its kind may change. A property of an object or a struct
/// can hold anything. A container's items, an option's value and a map's values cannot: the file
/// declares their kind once, ahead of the values, so writing a value of a different kind there
/// produces a file the game cannot read. A slot carries that constraint along with the borrow,
/// which is what lets [`ValueSlot::set`] check it.
///
/// Editing a value in place can never change its kind, so [`ValueSlot::as_mut`] and
/// [`ValueSlot::get_mut`] hand out the concrete value type with no check at all.
///
/// # Examples
///
/// ```
/// use ltk_meta::property::{values, Kind, ValueMut};
///
/// let mut list: values::Container = vec![values::I32::new(1), values::I32::new(2)].into();
/// let mut slot = list.slot(0).unwrap();
///
/// // The list declared `i32`, so that is what this slot will accept.
/// assert_eq!(slot.pinned_kind(), Some(Kind::I32));
/// assert!(slot.set(values::String::from("no").into()).is_err());
/// assert_eq!(slot.set(values::I32::new(7).into())?, values::I32::new(1).into());
///
/// // Editing in place needs no check, because it cannot change the kind.
/// if let ValueMut::I32(i) = slot.as_mut() {
///     i.value += 1;
/// }
/// assert_eq!(list.get(0), Some(&values::I32::new(8).into()));
/// # Ok::<(), ltk_meta::Error>(())
/// ```
#[derive(Debug, PartialEq)]
pub struct ValueSlot<'a, M = NoMeta> {
    pinned: Option<Kind>,
    value: &'a mut PropertyValueEnum<M>,
}

impl<'a, M> ValueSlot<'a, M> {
    /// A slot whose holder declared `kind` and accepts nothing else.
    pub(crate) fn pinned(kind: Kind, value: &'a mut PropertyValueEnum<M>) -> Self {
        Self {
            pinned: Some(kind),
            value,
        }
    }

    /// A slot whose holder accepts a value of any kind.
    pub(crate) fn free(value: &'a mut PropertyValueEnum<M>) -> Self {
        Self {
            pinned: None,
            value,
        }
    }

    /// The borrow itself, for walking further down inside the crate.
    pub(crate) fn into_inner(self) -> &'a mut PropertyValueEnum<M> {
        self.value
    }

    /// The kind of the value currently in the slot.
    #[inline(always)]
    #[must_use]
    pub fn kind(&self) -> Kind {
        self.value.kind()
    }

    /// The kind a replacement has to have, or `None` when the holder accepts any.
    #[inline(always)]
    #[must_use]
    pub fn pinned_kind(&self) -> Option<Kind> {
        self.pinned
    }

    /// The value in the slot.
    #[inline(always)]
    #[must_use]
    pub fn get(&self) -> &PropertyValueEnum<M> {
        self.value
    }

    /// A mutable borrow of the value that cannot change its kind.
    ///
    /// See [`PropertyValueEnum::as_mut`].
    #[inline(always)]
    pub fn as_mut(&mut self) -> ValueMut<'_, M> {
        self.value.as_mut()
    }

    /// See [`ValueSlot::as_mut`]. This reaches one concrete value type without a `match`.
    #[inline(always)]
    #[must_use]
    pub fn get_mut<T: FromValue<M>>(&mut self) -> Option<&mut T> {
        self.value.get_mut()
    }

    /// Replaces the value, returning the old one.
    ///
    /// # Errors
    ///
    /// [`Error::MismatchedContainerTypes`] when the holder pins a kind and `value` is not it.
    pub fn set(&mut self, value: PropertyValueEnum<M>) -> Result<PropertyValueEnum<M>, Error> {
        if let Some(pinned) = self.pinned {
            if value.kind() != pinned {
                return Err(Error::MismatchedContainerTypes {
                    expected: pinned,
                    got: value.kind(),
                });
            }
        }

        Ok(std::mem::replace(self.value, value))
    }
}
