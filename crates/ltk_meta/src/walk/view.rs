//! The streaming view as the walk sees it: [`ValueView`] and [`StructView`].

use std::fmt;

use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, Child, Leaf, TreeKind as _, TreeNode, TreeValue},
    Error,
};
use crate::{
    property::{values, Kind},
    stream::{ContainerItems, MapEntries, Properties, PropertyView, StructView, ValueView},
    PropertyValueEnum,
};

impl Sealed for ViewValue<'_> {}
impl Sealed for StructView<'_> {}

/// A borrowed walk value whose property payload is decoded only on request.
///
/// Property callbacks receive this adapter. [`TreeValue::kind`] reads the property header;
/// [`TreeValue::leaf`] and [`TreeValue::to_value`] decode the payload.
pub struct ViewValue<'a> {
    inner: ViewValueInner<'a>,
}

enum ViewValueInner<'a> {
    Property(PropertyView<'a>),
    Decoded(ValueView<'a>),
}

impl Copy for ViewValue<'_> {}
impl Clone for ViewValue<'_> {
    fn clone(&self) -> Self {
        *self
    }
}
impl Copy for ViewValueInner<'_> {}
impl Clone for ViewValueInner<'_> {
    fn clone(&self) -> Self {
        *self
    }
}

impl fmt::Debug for ViewValue<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            ViewValueInner::Property(p) => f.debug_tuple("ViewValue").field(&p).finish(),
            ViewValueInner::Decoded(v) => f.debug_tuple("ViewValue").field(&v).finish(),
        }
    }
}

impl<'a> ViewValue<'a> {
    fn property(property: PropertyView<'a>) -> Self {
        Self {
            inner: ViewValueInner::Property(property),
        }
    }

    fn decoded(value: ValueView<'a>) -> Self {
        Self {
            inner: ViewValueInner::Decoded(value),
        }
    }

    fn decode(&self) -> Result<ValueView<'a>, Error> {
        match self.inner {
            ViewValueInner::Property(p) => p.value_view(),
            ViewValueInner::Decoded(v) => Ok(v),
        }
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

/// The values inside a viewed container, optional or map, each decoded as it is reached.
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ViewChildren<'a> {
    inner: ViewChildrenInner<'a>,
}

enum ViewChildrenInner<'a> {
    Items {
        items: ContainerItems<'a>,
        index: usize,
    },
    Optional(Option<ValueView<'a>>),
    Entries(MapEntries<'a>),
    Empty,
}

impl<'a> Iterator for ViewChildren<'a> {
    type Item = Result<(Child<ViewValue<'a>>, ViewValue<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            ViewChildrenInner::Items { items, index } => {
                let item = items.next()?;
                let step = Child::Index(*index);
                *index += 1;
                Some(item.map(|value| (step, ViewValue::decoded(value))))
            }
            ViewChildrenInner::Optional(value) => value
                .take()
                .map(|v| Ok((Child::Index(0), ViewValue::decoded(v)))),
            ViewChildrenInner::Entries(entries) => {
                let entry = entries.next()?;
                Some(entry.map(|(key, value)| {
                    (
                        Child::Key(ViewValue::decoded(key)),
                        ViewValue::decoded(value),
                    )
                }))
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

impl std::iter::FusedIterator for ViewChildren<'_> {}

impl fmt::Debug for ViewChildren<'_> {
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

impl<'a> TreeValue<'a> for ViewValue<'a> {
    type Node = StructView<'a>;
    type Children = ViewChildren<'a>;

    fn kind(&self) -> Kind {
        match self.inner {
            ViewValueInner::Property(p) => p.kind(),
            ViewValueInner::Decoded(v) => v.kind(),
        }
    }

    fn holds_node(&self) -> Result<bool, Error> {
        if !self.kind().is_node() && !self.kind().is_container() {
            return Ok(false);
        }
        Ok(match self.decode()? {
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
        Ok(match self.decode()? {
            ValueView::Struct(s) | ValueView::Embedded(s) if *s.class_hash() != 0 => Some(s),
            _ => None,
        })
    }

    fn children(&self) -> Result<Self::Children, Error> {
        if !self.kind().is_container() {
            return Ok(ViewChildren {
                inner: ViewChildrenInner::Empty,
            });
        }
        let inner = match self.decode()? {
            ValueView::Container(c) | ValueView::UnorderedContainer(c) => {
                ViewChildrenInner::Items {
                    items: c.iter(),
                    index: 0,
                }
            }
            ValueView::Optional(o) => ViewChildrenInner::Optional(o.get()?),
            ValueView::Map(m) => ViewChildrenInner::Entries(m.iter()),
            _ => ViewChildrenInner::Empty,
        };
        Ok(ViewChildren { inner })
    }

    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
        Ok(Some(match self.decode()? {
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
        use PropertyValueEnum as P;
        macro_rules! prim {
            ($ty:ident, $v:expr) => {
                P::$ty(values::$ty::new($v))
            };
        }
        Ok(match self.decode()? {
            ValueView::None => P::None(values::None),
            ValueView::Bool(v) => prim!(Bool, v),
            ValueView::I8(v) => prim!(I8, v),
            ValueView::U8(v) => prim!(U8, v),
            ValueView::I16(v) => prim!(I16, v),
            ValueView::U16(v) => prim!(U16, v),
            ValueView::I32(v) => prim!(I32, v),
            ValueView::U32(v) => prim!(U32, v),
            ValueView::I64(v) => prim!(I64, v),
            ValueView::U64(v) => prim!(U64, v),
            ValueView::F32(v) => prim!(F32, v),
            ValueView::Vector2(v) => prim!(Vector2, v),
            ValueView::Vector3(v) => prim!(Vector3, v),
            ValueView::Vector4(v) => prim!(Vector4, v),
            ValueView::Matrix44(v) => prim!(Matrix44, v),
            ValueView::Color(v) => prim!(Color, v),
            ValueView::String(v) => prim!(String, v.to_owned()),
            ValueView::Hash(v) => prim!(Hash, v),
            ValueView::WadChunkLink(v) => prim!(WadChunkLink, v),
            ValueView::ObjectLink(v) => prim!(ObjectLink, v),
            ValueView::BitBool(v) => prim!(BitBool, v),
            ValueView::Struct(s) => P::Struct(struct_of(s)?),
            ValueView::Embedded(s) => P::Embedded(values::Embedded(struct_of(s)?)),
            ValueView::Container(c) => P::Container(container_of(c.item_kind(), c.iter())?),
            ValueView::UnorderedContainer(c) => P::UnorderedContainer(values::UnorderedContainer(
                container_of(c.item_kind(), c.iter())?,
            )),
            ValueView::Optional(o) => P::Optional(values::Optional::new(
                o.item_kind(),
                o.get()?
                    .map(|v| ViewValue::decoded(v).to_value())
                    .transpose()?,
            )?),
            ValueView::Map(m) => P::Map(values::Map::new(
                m.key_kind(),
                m.value_kind(),
                m.iter()
                    .map(|entry| {
                        let (k, v) = entry?;
                        Ok((
                            ViewValue::decoded(k).to_value()?,
                            ViewValue::decoded(v).to_value()?,
                        ))
                    })
                    .collect::<Result<_, Error>>()?,
            )?),
        })
    }
}

/// A null pointer stays a null pointer: class 0 and no properties.
fn struct_of(view: StructView<'_>) -> Result<values::Struct, Error> {
    if *view.class_hash() == 0 {
        return Ok(values::Struct::default());
    }
    view.to_struct()
}

fn container_of(item_kind: Kind, items: ContainerItems<'_>) -> Result<values::Container, Error> {
    values::Container::new(
        item_kind,
        items
            .map(|item| ViewValue::decoded(item?).to_value())
            .collect::<Result<_, Error>>()?,
    )
}
