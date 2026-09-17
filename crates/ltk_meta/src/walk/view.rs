//! The streaming view as the walk sees it: [`ValueView`] and [`StructView`].

use std::fmt;

use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, Child, Leaf, TreeKind as _, TreeNode, TreeValue},
    Error,
};
use crate::{
    property::{values, Kind},
    stream::{
        layout::Cursor,
        owned,
        view::value::{EntryCursors, ItemCursors},
        Properties, PropertyView, StructView, ValueView,
    },
    PropertyValueEnum,
};

impl Sealed for ViewValue<'_> {}
impl Sealed for StructView<'_> {}

/// A borrowed walk value: a kind, and the bytes the value is written in.
///
/// Nothing is decoded until a method asks for it, wherever the value came from: a property, a
/// container item, a map key or a map value. [`TreeValue::kind`] reads nothing.
/// [`TreeValue::leaf`] and [`TreeValue::to_value`] decode the bytes.
pub struct ViewValue<'a> {
    kind: Kind,
    at: Cursor<'a>,
}

impl Copy for ViewValue<'_> {}
impl Clone for ViewValue<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for ViewValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewValue")
            .field("kind", &self.kind)
            .field("bytes", &self.at.rest().len())
            .finish()
    }
}

impl<'a> ViewValue<'a> {
    /// A value of `kind` written at `at`, the header included.
    fn new(kind: Kind, at: Cursor<'a>) -> Self {
        Self { kind, at }
    }

    fn property(property: PropertyView<'a>) -> Self {
        Self::new(property.kind(), property.cursor())
    }

    /// The borrowed streaming view of this value.
    ///
    /// Leaf payloads decode on request. Complex values expose headers without decoding
    /// their contents. The view borrows the source bytes and allocates nothing.
    ///
    /// # Errors
    ///
    /// A header or leaf payload that does not decode.
    pub fn value_view(&self) -> Result<ValueView<'a>, Error> {
        let mut at = self.at;
        ValueView::read(&mut at, self.kind)
    }
}

/// The properties of a [`StructView`], in file order, each header decoded as it is reached.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewProperties<'a> {
    inner: Properties<'a>,
}

impl<'a> Iterator for ViewProperties<'a> {
    type Item = Result<(BinHash, ViewValue<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let property = self.inner.next()?;
        Some(property.map(|p| (p.name_hash(), ViewValue::property(p))))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl std::iter::FusedIterator for ViewProperties<'_> {}

impl fmt::Debug for ViewProperties<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewProperties")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a> TreeNode<'a> for StructView<'a> {
    type Value = ViewValue<'a>;
    type Properties = ViewProperties<'a>;

    fn class_hash(&self) -> BinHash {
        StructView::class_hash(self)
    }

    fn properties(&self) -> Self::Properties {
        ViewProperties {
            inner: StructView::properties(self),
        }
    }

    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error> {
        Ok(StructView::property(self, field)?.map(ViewValue::property))
    }

    fn to_struct(&self) -> Result<values::Struct, Error> {
        Ok(values::Struct {
            class_hash: StructView::class_hash(self),
            properties: TreeNode::properties(self)
                .map(|property| {
                    let (field, value) = property?;
                    Ok((field, value.to_value()?))
                })
                .collect::<Result<_, Error>>()?,
        })
    }
}

/// The values inside a viewed container, optional or map, each left undecoded.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewChildren<'a> {
    inner: ViewChildrenInner<'a>,
}

enum ViewChildrenInner<'a> {
    Items {
        items: ItemCursors<'a>,
        item_kind: Kind,
        index: usize,
    },
    Optional {
        value: Option<Cursor<'a>>,
        item_kind: Kind,
    },
    Entries {
        entries: EntryCursors<'a>,
        key_kind: Kind,
        value_kind: Kind,
    },
    Empty,
}

impl<'a> Iterator for ViewChildren<'a> {
    type Item = Result<(Child<ViewValue<'a>>, ViewValue<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            ViewChildrenInner::Items {
                items,
                item_kind,
                index,
            } => {
                let item = items.next()?;
                let step = Child::Index(*index);
                *index += 1;
                Some(item.map(|at| (step, ViewValue::new(*item_kind, at))))
            }
            ViewChildrenInner::Optional { value, item_kind } => value
                .take()
                .map(|at| Ok((Child::Index(0), ViewValue::new(*item_kind, at)))),
            ViewChildrenInner::Entries {
                entries,
                key_kind,
                value_kind,
            } => {
                let entry = entries.next()?;
                Some(entry.map(|(key, value)| {
                    (
                        Child::Key(ViewValue::new(*key_kind, key)),
                        ViewValue::new(*value_kind, value),
                    )
                }))
            }
            ViewChildrenInner::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            ViewChildrenInner::Items { items, .. } => items.size_hint(),
            ViewChildrenInner::Optional { value, .. } => {
                let n = usize::from(value.is_some());
                (n, Some(n))
            }
            ViewChildrenInner::Entries { entries, .. } => entries.size_hint(),
            ViewChildrenInner::Empty => (0, Some(0)),
        }
    }
}

impl std::iter::FusedIterator for ViewChildren<'_> {}

impl fmt::Debug for ViewChildren<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, remaining) = match &self.inner {
            ViewChildrenInner::Items { items, .. } => ("items", items.size_hint().0),
            ViewChildrenInner::Optional { value, .. } => ("optional", usize::from(value.is_some())),
            ViewChildrenInner::Entries { entries, .. } => ("entries", entries.size_hint().0),
            ViewChildrenInner::Empty => ("empty", 0),
        };
        f.debug_struct("ViewChildren")
            .field("kind", &kind)
            .field("remaining", &remaining)
            .finish()
    }
}

impl<'a> TreeValue<'a> for ViewValue<'a> {
    type Node = StructView<'a>;
    type Children = ViewChildren<'a>;

    fn kind(&self) -> Kind {
        self.kind
    }

    fn holds_node(&self) -> Result<bool, Error> {
        if !self.kind().is_node() && !self.kind().is_container() {
            return Ok(false);
        }
        Ok(match self.value_view()? {
            ValueView::Struct(s) | ValueView::Embedded(s) => *s.class_hash() != 0,
            ValueView::Container(c) | ValueView::UnorderedContainer(c) => c.item_kind().is_node(),
            ValueView::Optional(o) => o.item_kind().is_node(),
            ValueView::Map(m) => m.value_kind().is_node(),
            _ => false,
        })
    }

    fn as_node(&self) -> Result<Option<Self::Node>, Error> {
        if !self.kind().is_node() {
            return Ok(None);
        }
        Ok(match self.value_view()? {
            ValueView::Struct(s) | ValueView::Embedded(s) if *s.class_hash() != 0 => Some(s),
            _ => None,
        })
    }

    fn children(&self) -> Result<Self::Children, Error> {
        let children = |inner| ViewChildren { inner };
        if !self.kind().is_container() {
            return Ok(children(ViewChildrenInner::Empty));
        }
        let inner = match self.value_view()? {
            ValueView::Container(c) | ValueView::UnorderedContainer(c) => {
                ViewChildrenInner::Items {
                    items: c.cursors(),
                    item_kind: c.item_kind(),
                    index: 0,
                }
            }
            ValueView::Optional(o) => ViewChildrenInner::Optional {
                value: o.cursor(),
                item_kind: o.item_kind(),
            },
            ValueView::Map(m) => ViewChildrenInner::Entries {
                entries: m.cursors(),
                key_kind: m.key_kind(),
                value_kind: m.value_kind(),
            },
            _ => ViewChildrenInner::Empty,
        };
        Ok(children(inner))
    }

    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
        Ok(Some(match self.value_view()? {
            ValueView::None => Leaf::None,
            ValueView::Bool(v) => Leaf::Bool(v),
            ValueView::I8(v) => Leaf::I8(v),
            ValueView::U8(v) => Leaf::U8(v),
            ValueView::I16(v) => Leaf::I16(v),
            ValueView::U16(v) => Leaf::U16(v),
            ValueView::I32(v) => Leaf::I32(v),
            ValueView::U32(v) => Leaf::U32(v),
            ValueView::I64(v) => Leaf::I64(v),
            ValueView::U64(v) => Leaf::U64(v),
            ValueView::F32(v) => Leaf::F32(v),
            ValueView::Vector2(v) => Leaf::Vector2(v),
            ValueView::Vector3(v) => Leaf::Vector3(v),
            ValueView::Vector4(v) => Leaf::Vector4(v),
            ValueView::Matrix44(v) => Leaf::Matrix44(v),
            ValueView::Color(v) => Leaf::Color(v),
            ValueView::String(v) => Leaf::String(v),
            ValueView::Hash(v) => Leaf::Hash(v),
            ValueView::WadChunkLink(v) => Leaf::File(v),
            ValueView::ObjectLink(v) => Leaf::Link(v),
            ValueView::BitBool(v) => Leaf::Flag(v),
            ValueView::Container(_)
            | ValueView::UnorderedContainer(_)
            | ValueView::Optional(_)
            | ValueView::Map(_)
            | ValueView::Struct(_)
            | ValueView::Embedded(_) => return Ok(None),
        }))
    }

    fn to_value(&self) -> Result<PropertyValueEnum, Error> {
        let mut at = self.at;
        owned::read_value(&mut at, self.kind)
    }
}
