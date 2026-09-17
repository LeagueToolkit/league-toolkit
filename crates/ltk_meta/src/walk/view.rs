//! The streaming view as the walk sees it: [`RawValue`] and [`StructView`].

use std::{fmt, marker::PhantomData};

use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, ChildSegment, Declaration, Leaf, TreeKind as _, TreeNode, TreeValue},
    Error,
};
use crate::{
    property::{values, Kind, NoMeta},
    stream::{
        layout::Cursor,
        owned,
        view::value::{EntryCursors, ItemCursors},
        Properties, PropertyView, StructView, ValueView,
    },
    PropertyValueEnum,
};

impl<M> Sealed for RawValue<'_, M> {}
impl<M> Sealed for StructView<'_, M> {}

/// A borrowed walk value: a kind, and the bytes the value is written in.
///
/// Nothing is decoded until a method asks for it, wherever the value came from: a property, a
/// container item, a map key or a map value. [`TreeValue::kind`] reads nothing.
/// [`TreeValue::as_leaf`] and [`TreeValue::to_value`] decode the bytes.
pub struct RawValue<'a, M = NoMeta> {
    kind: Kind,
    at: Cursor<'a>,
    meta: PhantomData<fn() -> M>,
}

impl<M> Copy for RawValue<'_, M> {}
impl<M> Clone for RawValue<'_, M> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<M> fmt::Debug for RawValue<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawValue")
            .field("kind", &self.kind)
            .field("bytes", &self.at.rest().len())
            .finish()
    }
}

impl<'a, M> RawValue<'a, M> {
    /// A value of `kind` written at `at`, the header included.
    fn new(kind: Kind, at: Cursor<'a>) -> Self {
        Self {
            kind,
            at,
            meta: PhantomData,
        }
    }

    fn property(property: PropertyView<'a, M>) -> Self {
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
    pub fn value_view(&self) -> Result<ValueView<'a, M>, Error> {
        let mut at = self.at;
        ValueView::read(&mut at, self.kind)
    }
}

/// The properties of a [`StructView`], in file order, each header decoded as it is reached.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewProperties<'a, M = NoMeta> {
    inner: Properties<'a, M>,
}

impl<'a, M> Iterator for ViewProperties<'a, M> {
    type Item = Result<(BinHash, RawValue<'a, M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let property = self.inner.next()?;
        Some(property.map(|p| (p.name_hash(), RawValue::property(p))))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<M> std::iter::FusedIterator for ViewProperties<'_, M> {}

impl<M> fmt::Debug for ViewProperties<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ViewProperties")
            .field("inner", &self.inner)
            .finish()
    }
}

impl<'a, M: Default> TreeNode<'a> for StructView<'a, M> {
    type Value = RawValue<'a, M>;
    type Properties = ViewProperties<'a, M>;

    fn class_hash(&self) -> BinHash {
        StructView::class_hash(self)
    }

    fn properties(&self) -> Self::Properties {
        ViewProperties {
            inner: StructView::properties(self),
        }
    }

    fn get(&self, field: BinHash) -> Result<Option<Self::Value>, Error> {
        Ok(StructView::property(self, field)?.map(RawValue::property))
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
            meta: NoMeta,
        })
    }
}

/// The values inside a viewed container, optional or map, each left undecoded.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewChildren<'a, M = NoMeta> {
    inner: ViewChildrenInner<'a>,
    meta: PhantomData<fn() -> M>,
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

impl<'a, M> Iterator for ViewChildren<'a, M> {
    type Item = Result<(ChildSegment<RawValue<'a, M>>, RawValue<'a, M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            ViewChildrenInner::Items {
                items,
                item_kind,
                index,
            } => {
                let item = items.next()?;
                let segment = ChildSegment::Index(*index);
                *index += 1;
                Some(item.map(|at| (segment, RawValue::new(*item_kind, at))))
            }
            ViewChildrenInner::Optional { value, item_kind } => value
                .take()
                .map(|at| Ok((ChildSegment::Index(0), RawValue::new(*item_kind, at)))),
            ViewChildrenInner::Entries {
                entries,
                key_kind,
                value_kind,
            } => {
                let entry = entries.next()?;
                Some(entry.map(|(key, value)| {
                    (
                        ChildSegment::Key(RawValue::new(*key_kind, key)),
                        RawValue::new(*value_kind, value),
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

impl<M> std::iter::FusedIterator for ViewChildren<'_, M> {}

impl<M> fmt::Debug for ViewChildren<'_, M> {
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

impl<'a, M: Default> TreeValue<'a> for RawValue<'a, M> {
    type Node = StructView<'a, M>;
    type Children = ViewChildren<'a, M>;

    fn kind(&self) -> Kind {
        self.kind
    }

    fn declaration(&self) -> Result<Declaration, Error> {
        let mut declaration = Declaration::bare(self.kind());
        if !declaration.kind.is_node() && !declaration.kind.is_container() {
            return Ok(declaration);
        }
        match self.value_view()? {
            ValueView::Struct(node) | ValueView::Embedded(node) => {
                declaration.class = Some(node.class_hash());
            }
            ValueView::Container(items) | ValueView::UnorderedContainer(items) => {
                declaration.item_kind = Some(items.item_kind());
                declaration.count = Some(items.len() as usize);
            }
            ValueView::Optional(option) => {
                declaration.item_kind = Some(option.item_kind());
                declaration.count = Some(usize::from(option.is_some()));
            }
            ValueView::Map(map) => {
                declaration.item_kind = Some(map.value_kind());
                declaration.key_kind = Some(map.key_kind());
                declaration.count = Some(map.len() as usize);
            }
            _ => {}
        }
        Ok(declaration)
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
        let children = |inner| ViewChildren {
            inner,
            meta: PhantomData,
        };
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

    fn as_leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
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
