use crate::{property::values::Container, traits::PropertyValueDyn, PropertyValueEnum};

pub struct ItemsDyn<'a>(ItemsDynInner<'a>);
impl<'a> Iterator for ItemsDyn<'a> {
    type Item = &'a dyn PropertyValueDyn;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
impl<'a> From<ItemsDynInner<'a>> for ItemsDyn<'a> {
    fn from(value: ItemsDynInner<'a>) -> Self {
        Self(value)
    }
}

pub struct IntoItems(IntoItemsInner);
impl Iterator for IntoItems {
    type Item = PropertyValueEnum;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}
impl From<IntoItemsInner> for IntoItems {
    fn from(value: IntoItemsInner) -> Self {
        Self(value)
    }
}

macro_rules! define_dyn_iter {
    ( $( $variant:ident($ty:ty), )* ) => {
        enum ItemsDynInner<'a> {
            $($variant(std::slice::Iter<'a, $ty>),)*
        }

        impl<'a> Iterator for ItemsDynInner<'a> {
            type Item = &'a dyn PropertyValueDyn;

            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $(Self::$variant(it) => it.next().map(|x| x as _),)*
                }
            }
        }

        $(
            impl<'a> From<std::slice::Iter<'a, $ty>> for ItemsDynInner<'a> {
                fn from(other: std::slice::Iter<'a, $ty>) -> Self {
                    Self::$variant(other)
                }
            }
        )*


        enum IntoItemsInner {
            $($variant(std::vec::IntoIter<$ty>),)*
        }
        impl Iterator for IntoItemsInner {
            type Item = PropertyValueEnum;

            fn next(&mut self) -> Option<Self::Item> {
                match self {
                    $(Self::$variant(it) => it.next().map(|x| x.into()),)*
                }
            }
        }

        $(
            impl From<std::vec::IntoIter<$ty>> for IntoItemsInner {
                fn from(other: std::vec::IntoIter<$ty>) -> Self {
                    Self::$variant(other)
                }
            }
        )*

    };
}

variants!(define_dyn_iter);

impl Container {
    /// Iterator that returns each item as a [`PropertyValueEnum`] for convenience.
    #[inline(always)]
    #[must_use]
    pub fn into_items(self) -> IntoItems {
        match_property!(self, |inner| {
            IntoItems::from(IntoItemsInner::from(inner.into_iter()))
        })
    }
    /// Iterator over each item as a dyn [`PropertyValueDyn`].
    #[inline(always)]
    #[must_use]
    pub fn items_dyn(&self) -> ItemsDyn<'_> {
        match_property!(&self, |items| { ItemsDynInner::from(items.iter()) }).into()
    }
}
