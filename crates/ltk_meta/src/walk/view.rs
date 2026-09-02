//! The streaming view as the walk sees it: [`ValueView`] and [`StructView`].

use std::fmt;

use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, Child, Leaf, TreeNode, TreeValue},
    Error,
};
use crate::{
    property::{values, Kind, NoMeta},
    stream::{ContainerItems, MapEntries, Properties, StructView, ValueView},
    PropertyValueEnum,
};

impl<M> Sealed for ValueView<'_, M> {}
impl<M> Sealed for StructView<'_, M> {}

/// The properties of a [`StructView`], in file order, each header decoded as it is reached.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewProperties<'a, M = NoMeta> {
    inner: Properties<'a, M>,
}

impl<'a, M> Iterator for ViewProperties<'a, M> {
    type Item = Result<(BinHash, ValueView<'a, M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let property = self.inner.next()?;
        Some(property.and_then(|p| Ok((p.name_hash(), p.value_view()?))))
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
    type Value = ValueView<'a, M>;
    type Properties = ViewProperties<'a, M>;

    fn class_hash(&self) -> BinHash {
        StructView::class_hash(self)
    }

    fn properties(&self) -> Self::Properties {
        ViewProperties {
            inner: StructView::properties(self),
        }
    }

    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error> {
        StructView::property(self, field)?
            .map(|p| p.value_view())
            .transpose()
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

/// The values inside a viewed container, optional or map, each decoded as it is reached.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewChildren<'a, M = NoMeta> {
    inner: ViewChildrenInner<'a, M>,
}

enum ViewChildrenInner<'a, M> {
    Items {
        items: ContainerItems<'a, M>,
        index: usize,
    },
    Optional(Option<ValueView<'a, M>>),
    Entries(MapEntries<'a, M>),
    Empty,
}

impl<'a, M> Iterator for ViewChildren<'a, M> {
    type Item = Result<(Child<ValueView<'a, M>>, ValueView<'a, M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            ViewChildrenInner::Items { items, index } => {
                let item = items.next()?;
                let step = Child::Index(*index);
                *index += 1;
                Some(item.map(|value| (step, value)))
            }
            ViewChildrenInner::Optional(value) => value.take().map(|v| Ok((Child::Index(0), v))),
            ViewChildrenInner::Entries(entries) => {
                let entry = entries.next()?;
                Some(entry.map(|(key, value)| (Child::Key(key), value)))
            }
            ViewChildrenInner::Empty => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            ViewChildrenInner::Items { items, .. } => items.size_hint(),
            ViewChildrenInner::Optional(value) => {
                let n = usize::from(value.is_some());
                (n, Some(n))
            }
            ViewChildrenInner::Entries(entries) => entries.size_hint(),
            ViewChildrenInner::Empty => (0, Some(0)),
        }
    }
}

impl<M> std::iter::FusedIterator for ViewChildren<'_, M> {}

impl<M> fmt::Debug for ViewChildren<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, remaining) = match &self.inner {
            ViewChildrenInner::Items { items, .. } => ("items", items.size_hint().0),
            ViewChildrenInner::Optional(value) => ("optional", usize::from(value.is_some())),
            ViewChildrenInner::Entries(entries) => ("entries", entries.size_hint().0),
            ViewChildrenInner::Empty => ("empty", 0),
        };
        f.debug_struct("ViewChildren")
            .field("kind", &kind)
            .field("remaining", &remaining)
            .finish()
    }
}

impl<'a, M: Default> TreeValue<'a> for ValueView<'a, M> {
    type Node = StructView<'a, M>;
    type Children = ViewChildren<'a, M>;

    fn kind(&self) -> Kind {
        ValueView::kind(self)
    }

    fn holds_node(&self) -> Result<bool, Error> {
        Ok(match self {
            Self::Struct(s) | Self::Embedded(s) => *s.class_hash() != 0,
            Self::Container(c) | Self::UnorderedContainer(c) => c.item_kind().is_node(),
            Self::Optional(o) => o.item_kind().is_node(),
            Self::Map(m) => m.value_kind().is_node(),
            _ => false,
        })
    }

    fn as_node(&self) -> Result<Option<Self::Node>, Error> {
        Ok(match self {
            Self::Struct(s) | Self::Embedded(s) if *s.class_hash() != 0 => Some(*s),
            _ => None,
        })
    }

    fn children(&self) -> Result<Self::Children, Error> {
        let inner = match self {
            Self::Container(c) | Self::UnorderedContainer(c) => ViewChildrenInner::Items {
                items: c.iter(),
                index: 0,
            },
            Self::Optional(o) => ViewChildrenInner::Optional(o.get()?),
            Self::Map(m) => ViewChildrenInner::Entries(m.iter()),
            _ => ViewChildrenInner::Empty,
        };
        Ok(ViewChildren { inner })
    }

    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
        Ok(Some(match *self {
            Self::None => Leaf::None,
            Self::Bool(v) => Leaf::Bool(v),
            Self::I8(v) => Leaf::I8(v),
            Self::U8(v) => Leaf::U8(v),
            Self::I16(v) => Leaf::I16(v),
            Self::U16(v) => Leaf::U16(v),
            Self::I32(v) => Leaf::I32(v),
            Self::U32(v) => Leaf::U32(v),
            Self::I64(v) => Leaf::I64(v),
            Self::U64(v) => Leaf::U64(v),
            Self::F32(v) => Leaf::F32(v),
            Self::Vector2(v) => Leaf::Vector2(v),
            Self::Vector3(v) => Leaf::Vector3(v),
            Self::Vector4(v) => Leaf::Vector4(v),
            Self::Matrix44(v) => Leaf::Matrix44(v),
            Self::Color(v) => Leaf::Color(v),
            Self::String(v) => Leaf::String(v),
            Self::Hash(v) => Leaf::Hash(v),
            Self::WadChunkLink(v) => Leaf::File(v),
            Self::ObjectLink(v) => Leaf::Link(v),
            Self::BitBool(v) => Leaf::Flag(v),
            Self::Container(_)
            | Self::UnorderedContainer(_)
            | Self::Optional(_)
            | Self::Map(_)
            | Self::Struct(_)
            | Self::Embedded(_) => return Ok(None),
        }))
    }

    fn to_value(&self) -> Result<PropertyValueEnum, Error> {
        use PropertyValueEnum as P;
        macro_rules! prim {
            ($ty:ident, $v:expr) => {
                P::$ty(values::$ty::new_with_meta($v, NoMeta))
            };
        }
        Ok(match *self {
            Self::None => P::None(values::None { meta: NoMeta }),
            Self::Bool(v) => prim!(Bool, v),
            Self::I8(v) => prim!(I8, v),
            Self::U8(v) => prim!(U8, v),
            Self::I16(v) => prim!(I16, v),
            Self::U16(v) => prim!(U16, v),
            Self::I32(v) => prim!(I32, v),
            Self::U32(v) => prim!(U32, v),
            Self::I64(v) => prim!(I64, v),
            Self::U64(v) => prim!(U64, v),
            Self::F32(v) => prim!(F32, v),
            Self::Vector2(v) => prim!(Vector2, v),
            Self::Vector3(v) => prim!(Vector3, v),
            Self::Vector4(v) => prim!(Vector4, v),
            Self::Matrix44(v) => prim!(Matrix44, v),
            Self::Color(v) => prim!(Color, v),
            Self::String(v) => prim!(String, v.to_owned()),
            Self::Hash(v) => prim!(Hash, v),
            Self::WadChunkLink(v) => prim!(WadChunkLink, v),
            Self::ObjectLink(v) => prim!(ObjectLink, v),
            Self::BitBool(v) => prim!(BitBool, v),
            Self::Struct(s) => P::Struct(struct_of(s)?),
            Self::Embedded(s) => P::Embedded(values::Embedded(struct_of(s)?)),
            Self::Container(c) => P::Container(container_of(c.item_kind(), c.iter())?),
            Self::UnorderedContainer(c) => P::UnorderedContainer(values::UnorderedContainer(
                container_of(c.item_kind(), c.iter())?,
            )),
            Self::Optional(o) => P::Optional(values::Optional::new(
                o.item_kind(),
                o.get()?.map(|v| v.to_value()).transpose()?,
            )?),
            Self::Map(m) => P::Map(values::Map::new(
                m.key_kind(),
                m.value_kind(),
                m.iter()
                    .map(|entry| {
                        let (k, v) = entry?;
                        Ok((k.to_value()?, v.to_value()?))
                    })
                    .collect::<Result<_, Error>>()?,
            )?),
        })
    }
}

/// A null pointer stays a null pointer: class 0 and no properties.
fn struct_of<M: Default>(view: StructView<'_, M>) -> Result<values::Struct, Error> {
    if *view.class_hash() == 0 {
        return Ok(values::Struct::default());
    }
    view.to_struct()
}

fn container_of<M: Default>(
    item_kind: Kind,
    items: ContainerItems<'_, M>,
) -> Result<values::Container, Error> {
    values::Container::new(
        item_kind,
        items
            .map(|item| item?.to_value())
            .collect::<Result<_, Error>>()?,
    )
}
