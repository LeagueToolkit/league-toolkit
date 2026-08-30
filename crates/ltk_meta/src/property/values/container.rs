use std::io;

use crate::{
    property::{Kind, NoMeta},
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty, WriterExt},
    Error, PropertyValueEnum, ValueSlot,
};
use byteorder::{WriteBytesExt, LE};
use ltk_io_ext::{measure, window_at};

mod item;
pub use item::ContainerItem;

/// A list of values that all have the same [`Kind`].
///
/// The kind is declared once, both in the file and in [`Container::item_kind`], and every item
/// matches it. [`Container::push`] and the checked constructors enforce that on the way in, and
/// [`Container::slot`] is the only way to reach an item mutably, which is why it pins the kind.
///
/// The format has no nested containers, so a container, option or map cannot be an item. The
/// checked constructors reject those kinds, and [`ContainerItem`] excludes them at compile time.
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Clone, Debug, PartialEq)]
pub struct Container<M = NoMeta> {
    item_kind: Kind,
    items: Vec<PropertyValueEnum<M>>,
    pub meta: M,
}

impl<M: Default> Container<M> {
    /// An empty container holding items of `item_kind`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `item_kind` is itself a container kind.
    pub fn empty(item_kind: Kind) -> Result<Self, Error> {
        Self::new(item_kind, Vec::new())
    }

    /// A container of `items`, all of which must be `item_kind`.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidNesting`] if `item_kind` is itself a container kind, or
    /// [`Error::MismatchedContainerTypes`] if an item is not `item_kind`.
    pub fn new(item_kind: Kind, items: Vec<PropertyValueEnum<M>>) -> Result<Self, Error> {
        if item_kind.is_container() {
            return Err(Error::InvalidNesting(item_kind));
        }
        for item in &items {
            if item.kind() != item_kind {
                return Err(Error::MismatchedContainerTypes {
                    expected: item_kind,
                    got: item.kind(),
                });
            }
        }

        Ok(Self {
            item_kind,
            items,
            meta: M::default(),
        })
    }
}

impl<M> Container<M> {
    /// The kind every item in this container has.
    #[inline(always)]
    #[must_use]
    pub fn item_kind(&self) -> Kind {
        self.item_kind
    }

    /// The items, in order.
    #[inline(always)]
    #[must_use]
    pub fn items(&self) -> &[PropertyValueEnum<M>] {
        &self.items
    }

    /// The item at `index`, if there is one.
    #[inline(always)]
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&PropertyValueEnum<M>> {
        self.items.get(index)
    }

    /// A mutable handle on item `index`, pinned to [`Container::item_kind`].
    ///
    /// There is no plain `&mut` to an item, because writing one of a different kind would leave
    /// the container inhomogeneous and [`Container::to_writer`] emitting a file the game cannot
    /// read. [`ValueSlot`] edits in place freely and checks the kind only on a whole-value
    /// replace.
    #[inline(always)]
    #[must_use]
    pub fn slot(&mut self, index: usize) -> Option<ValueSlot<'_, M>> {
        let item_kind = self.item_kind;
        Some(ValueSlot::pinned(item_kind, self.items.get_mut(index)?))
    }

    /// How many items this container holds.
    #[inline(always)]
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether this container holds no items.
    #[inline(always)]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Appends `value`.
    ///
    /// # Errors
    ///
    /// [`Error::MismatchedContainerTypes`] if `value` is not [`Container::item_kind`].
    pub fn push(&mut self, value: PropertyValueEnum<M>) -> Result<(), Error> {
        if value.kind() != self.item_kind {
            return Err(Error::MismatchedContainerTypes {
                expected: self.item_kind,
                got: value.kind(),
            });
        }

        self.items.push(value);
        Ok(())
    }

    /// The items, in order, by value.
    #[inline(always)]
    #[must_use]
    pub fn into_items(self) -> Vec<PropertyValueEnum<M>> {
        self.items
    }

    #[inline(always)]
    #[must_use]
    pub fn no_meta(self) -> Container<NoMeta> {
        Container {
            item_kind: self.item_kind,
            items: self.items.into_iter().map(|i| i.no_meta()).collect(),
            meta: NoMeta,
        }
    }
}

impl<M: Default> Default for Container<M> {
    fn default() -> Self {
        Self {
            item_kind: Kind::None,
            items: Vec::new(),
            meta: M::default(),
        }
    }
}

impl<M: Default, T: ContainerItem + Into<PropertyValueEnum<M>>> From<Vec<T>> for Container<M> {
    fn from(items: Vec<T>) -> Self {
        items.into_iter().collect()
    }
}

impl<M: Default, T: ContainerItem + Into<PropertyValueEnum<M>>> FromIterator<T> for Container<M> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        Self {
            item_kind: T::KIND,
            items: iter.into_iter().map(Into::into).collect(),
            meta: M::default(),
        }
    }
}

impl<M: Default> TryFrom<Vec<PropertyValueEnum<M>>> for Container<M> {
    type Error = Error;

    /// The item kind comes from the first item, so an empty vector has none to take.
    ///
    /// # Errors
    ///
    /// [`Error::EmptyContainer`] if `items` is empty, otherwise whatever [`Container::new`]
    /// returns.
    fn try_from(items: Vec<PropertyValueEnum<M>>) -> Result<Self, Self::Error> {
        let item_kind = items.first().ok_or(Error::EmptyContainer)?.kind();
        Self::new(item_kind, items)
    }
}

impl<M> PropertyExt for Container<M> {
    fn size_no_header(&self) -> usize {
        9 + self.items.iter().map(|i| i.size_no_header()).sum::<usize>()
    }

    type Meta = M;
    fn meta(&self) -> &Self::Meta {
        &self.meta
    }
    fn meta_mut(&mut self) -> &mut Self::Meta {
        &mut self.meta
    }
}

impl<M> PropertyValueExt for Container<M> {
    const KIND: Kind = Kind::Container;
}

impl<M: Default> ReadProperty for Container<M> {
    fn from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error> {
        owned::read_from(
            reader,
            Kind::Container,
            Numbering::from_legacy(legacy),
            owned::read_container,
        )
    }
}

impl<M: Clone> WriteProperty for Container<M> {
    // TODO: legacy writing
    fn to_writer<R: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), io::Error> {
        if legacy {
            unimplemented!("legacy container writing");
        }

        writer.write_property_kind(self.item_kind)?;
        let size_pos = writer.stream_position()?;
        writer.write_u32::<LE>(0)?;

        let (size, _) = measure(writer, |writer| {
            writer.write_u32::<LE>(self.items.len() as _)?;
            for item in &self.items {
                item.to_writer(writer)?;
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
    use crate::property::values;

    #[test]
    fn takes_its_item_kind_from_the_type() {
        let list = Container::<NoMeta>::from(vec![values::I32::new(1), values::I32::new(2)]);
        assert_eq!(list.item_kind(), Kind::I32);
        assert_eq!(list.len(), 2);

        // An empty iterator still knows what it would have held.
        let list: Container = std::iter::empty::<values::String>().collect();
        assert_eq!(list.item_kind(), Kind::String);
        assert!(list.is_empty());
    }

    #[test]
    fn infers_its_item_kind_from_the_first_item() {
        let items: Vec<PropertyValueEnum> =
            vec![values::U8::new(1).into(), values::U8::new(2).into()];
        let list: Container = Container::try_from(items).unwrap();
        assert_eq!(list.item_kind(), Kind::U8);

        // Nothing to infer from.
        let empty: Result<Container, _> = Container::try_from(Vec::<PropertyValueEnum>::new());
        assert!(matches!(empty, Err(Error::EmptyContainer)));
    }

    #[test]
    fn rejects_an_item_that_is_not_its_item_kind() {
        assert!(matches!(
            Container::<NoMeta>::new(Kind::I32, vec![values::String::from("no").into()]),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::I32,
                got: Kind::String
            })
        ));

        let mut list = Container::<NoMeta>::empty(Kind::I32).unwrap();
        assert!(matches!(
            list.push(values::String::from("no").into()),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::I32,
                got: Kind::String
            })
        ));
        assert!(list.push(values::I32::new(1).into()).is_ok());
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn rejects_a_nested_container() {
        // `ContainerItem` keeps the same kinds out of `from`/`from_iter` at compile time.
        for kind in [
            Kind::Container,
            Kind::UnorderedContainer,
            Kind::Optional,
            Kind::Map,
        ] {
            assert!(matches!(
                Container::<NoMeta>::empty(kind),
                Err(Error::InvalidNesting(k)) if k == kind
            ));
        }
    }

    #[test]
    fn borrows_its_items() {
        let list = Container::<NoMeta>::from(vec![values::I32::new(1), values::I32::new(2)]);

        assert_eq!(list.get(0), Some(&values::I32::new(1).into()));
        assert_eq!(list.get(2), None);
        assert_eq!(list.items().len(), 2);
        assert_eq!(list.into_items().len(), 2);
    }

    /// An item can be edited in place or replaced by its own kind, and by nothing else.
    #[test]
    fn pins_its_item_kind_to_the_slots_it_hands_out() {
        use crate::property::ValueMut;

        let mut list = Container::<NoMeta>::from(vec![values::I32::new(1), values::I32::new(2)]);
        assert!(list.slot(2).is_none());

        let mut slot = list.slot(1).unwrap();
        assert_eq!(slot.pinned_kind(), Some(Kind::I32));

        assert!(matches!(
            slot.set(values::String::from("no").into()),
            Err(Error::MismatchedContainerTypes {
                expected: Kind::I32,
                got: Kind::String
            })
        ));
        assert_eq!(
            slot.set(values::I32::new(7).into()).unwrap(),
            values::I32::new(2).into()
        );

        let ValueMut::I32(item) = slot.as_mut() else {
            panic!("the slot holds an i32");
        };
        item.value += 1;

        assert_eq!(list.item_kind(), Kind::I32);
        assert_eq!(
            list.items(),
            [values::I32::new(1).into(), values::I32::new(8).into()]
        );
    }
}
