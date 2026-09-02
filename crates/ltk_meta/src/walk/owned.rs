//! The owned tree as the walk sees it: `&PropertyValueEnum<M>` and [`OwnedNode`].

use std::{fmt, iter::Enumerate, slice};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, Child, Leaf, TreeNode, TreeValue},
    Error,
};
use crate::{
    property::Kind,
    property::{values, NoMeta},
    BinObject, PropertyValueEnum,
};

/// The owned tree's node: a class hash and a borrowed property map.
///
/// [`BinObject`] and [`values::Struct`] both view as one, through `From`.
pub struct OwnedNode<'a, M = NoMeta> {
    class_hash: BinHash,
    properties: &'a IndexMap<BinHash, PropertyValueEnum<M>>,
}

impl<'a, M> OwnedNode<'a, M> {
    /// A node over `properties`, carrying `class_hash`.
    #[must_use]
    pub fn new(
        class_hash: BinHash,
        properties: &'a IndexMap<BinHash, PropertyValueEnum<M>>,
    ) -> Self {
        Self {
            class_hash,
            properties,
        }
    }
}

impl<'a, M> From<&'a BinObject<M>> for OwnedNode<'a, M> {
    fn from(object: &'a BinObject<M>) -> Self {
        Self::new(object.class_hash, &object.properties)
    }
}

impl<'a, M> From<&'a values::Struct<M>> for OwnedNode<'a, M> {
    fn from(value: &'a values::Struct<M>) -> Self {
        Self::new(value.class_hash, &value.properties)
    }
}

// By hand rather than derived: a derived `Copy` would demand `M: Copy` for a borrow.
impl<M> Clone for OwnedNode<'_, M> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<M> Copy for OwnedNode<'_, M> {}

impl<M> fmt::Debug for OwnedNode<'_, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedNode")
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.properties.len())
            .finish()
    }
}

impl<M> Sealed for OwnedNode<'_, M> {}
impl<M> Sealed for &PropertyValueEnum<M> {}

/// The properties of an [`OwnedNode`], in order.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct OwnedProperties<'a, M = NoMeta> {
    inner: indexmap::map::Iter<'a, BinHash, PropertyValueEnum<M>>,
}

impl<'a, M> Iterator for OwnedProperties<'a, M> {
    type Item = Result<(BinHash, &'a PropertyValueEnum<M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(field, value)| Ok((*field, value)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl<M> ExactSizeIterator for OwnedProperties<'_, M> {}
impl<M> std::iter::FusedIterator for OwnedProperties<'_, M> {}

impl<'a, M> TreeNode<'a> for OwnedNode<'a, M> {
    type Value = &'a PropertyValueEnum<M>;
    type Properties = OwnedProperties<'a, M>;

    fn class_hash(&self) -> BinHash {
        self.class_hash
    }

    fn properties(&self) -> Self::Properties {
        OwnedProperties {
            inner: self.properties.iter(),
        }
    }

    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error> {
        Ok(self.properties.get(&field))
    }

    fn to_struct(&self) -> Result<values::Struct, Error> {
        Ok(values::Struct {
            class_hash: self.class_hash,
            properties: self
                .properties
                .iter()
                .map(|(field, value)| Ok((*field, strip_meta(value)?)))
                .collect::<Result<_, Error>>()?,
            meta: NoMeta,
        })
    }
}

/// The values inside an owned container, optional or map.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct OwnedChildren<'a, M = NoMeta> {
    inner: OwnedChildrenInner<'a, M>,
}

#[derive(Debug)]
enum OwnedChildrenInner<'a, M> {
    Items(Enumerate<slice::Iter<'a, PropertyValueEnum<M>>>),
    Entries(slice::Iter<'a, (PropertyValueEnum<M>, PropertyValueEnum<M>)>),
}

impl<'a, M> Iterator for OwnedChildren<'a, M> {
    type Item = Result<(Child<&'a PropertyValueEnum<M>>, &'a PropertyValueEnum<M>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            OwnedChildrenInner::Items(items) => items
                .next()
                .map(|(index, value)| Ok((Child::Index(index), value))),
            OwnedChildrenInner::Entries(entries) => entries
                .next()
                .map(|(key, value)| Ok((Child::Key(key), value))),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            OwnedChildrenInner::Items(items) => items.size_hint(),
            OwnedChildrenInner::Entries(entries) => entries.size_hint(),
        }
    }
}

impl<M> ExactSizeIterator for OwnedChildren<'_, M> {}
impl<M> std::iter::FusedIterator for OwnedChildren<'_, M> {}

impl<'a, M> TreeValue<'a> for &'a PropertyValueEnum<M> {
    type Node = OwnedNode<'a, M>;
    type Children = OwnedChildren<'a, M>;

    fn kind(&self) -> Kind {
        PropertyValueEnum::kind(self)
    }

    fn holds_node(&self) -> Result<bool, Error> {
        Ok(PropertyValueEnum::holds_node(self))
    }

    fn as_node(&self) -> Result<Option<Self::Node>, Error> {
        let node = match self {
            PropertyValueEnum::Struct(s) => s,
            PropertyValueEnum::Embedded(e) => &e.0,
            _ => return Ok(None),
        };
        Ok((*node.class_hash != 0).then(|| OwnedNode::from(node)))
    }

    fn children(&self) -> Result<Self::Children, Error> {
        let inner = match self {
            PropertyValueEnum::Container(c) => {
                OwnedChildrenInner::Items(c.items().iter().enumerate())
            }
            PropertyValueEnum::UnorderedContainer(c) => {
                OwnedChildrenInner::Items(c.0.items().iter().enumerate())
            }
            PropertyValueEnum::Optional(o) => OwnedChildrenInner::Items(
                o.value()
                    .map_or(&[][..], slice::from_ref)
                    .iter()
                    .enumerate(),
            ),
            PropertyValueEnum::Map(m) => OwnedChildrenInner::Entries(m.entries().iter()),
            _ => OwnedChildrenInner::Items([].iter().enumerate()),
        };
        Ok(OwnedChildren { inner })
    }

    fn leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
        use PropertyValueEnum as P;
        Ok(Some(match *self {
            P::None(_) => Leaf::None,
            P::Bool(v) => Leaf::Bool(v.value),
            P::I8(v) => Leaf::I8(v.value),
            P::U8(v) => Leaf::U8(v.value),
            P::I16(v) => Leaf::I16(v.value),
            P::U16(v) => Leaf::U16(v.value),
            P::I32(v) => Leaf::I32(v.value),
            P::U32(v) => Leaf::U32(v.value),
            P::I64(v) => Leaf::I64(v.value),
            P::U64(v) => Leaf::U64(v.value),
            P::F32(v) => Leaf::F32(v.value),
            P::Vector2(v) => Leaf::Vector2(v.value),
            P::Vector3(v) => Leaf::Vector3(v.value),
            P::Vector4(v) => Leaf::Vector4(v.value),
            P::Matrix44(v) => Leaf::Matrix44(v.value),
            P::Color(v) => Leaf::Color(v.value),
            P::String(v) => Leaf::String(&v.value),
            P::Hash(v) => Leaf::Hash(v.value),
            P::WadChunkLink(v) => Leaf::File(v.value),
            P::ObjectLink(v) => Leaf::Link(v.value),
            P::BitBool(v) => Leaf::Flag(v.value),
            P::Container(_)
            | P::UnorderedContainer(_)
            | P::Optional(_)
            | P::Map(_)
            | P::Struct(_)
            | P::Embedded(_) => return Ok(None),
        }))
    }

    fn to_value(&self) -> Result<PropertyValueEnum, Error> {
        strip_meta(self)
    }
}

/// A copy of `value` with every metadata slot reset to [`NoMeta`].
///
/// # Errors
///
/// Never: every container held by `value` satisfies the checks its constructor performs.
pub(crate) fn strip_meta<M>(value: &PropertyValueEnum<M>) -> Result<PropertyValueEnum, Error> {
    use PropertyValueEnum as P;
    macro_rules! prim {
        ($ty:ident, $v:expr) => {
            P::$ty(values::$ty::new_with_meta($v.value.clone(), NoMeta))
        };
    }
    Ok(match value {
        P::None(_) => P::None(values::None { meta: NoMeta }),
        P::Bool(v) => prim!(Bool, v),
        P::I8(v) => prim!(I8, v),
        P::U8(v) => prim!(U8, v),
        P::I16(v) => prim!(I16, v),
        P::U16(v) => prim!(U16, v),
        P::I32(v) => prim!(I32, v),
        P::U32(v) => prim!(U32, v),
        P::I64(v) => prim!(I64, v),
        P::U64(v) => prim!(U64, v),
        P::F32(v) => prim!(F32, v),
        P::Vector2(v) => prim!(Vector2, v),
        P::Vector3(v) => prim!(Vector3, v),
        P::Vector4(v) => prim!(Vector4, v),
        P::Matrix44(v) => prim!(Matrix44, v),
        P::Color(v) => prim!(Color, v),
        P::String(v) => prim!(String, v),
        P::Hash(v) => prim!(Hash, v),
        P::WadChunkLink(v) => prim!(WadChunkLink, v),
        P::ObjectLink(v) => prim!(ObjectLink, v),
        P::BitBool(v) => prim!(BitBool, v),
        P::Struct(s) => P::Struct(strip_struct(s)?),
        P::Embedded(e) => P::Embedded(values::Embedded(strip_struct(&e.0)?)),
        P::Container(c) => P::Container(strip_container(c)?),
        P::UnorderedContainer(c) => {
            P::UnorderedContainer(values::UnorderedContainer(strip_container(&c.0)?))
        }
        P::Optional(o) => P::Optional(values::Optional::new(
            o.item_kind(),
            o.value().map(strip_meta).transpose()?,
        )?),
        P::Map(m) => P::Map(values::Map::new(
            m.key_kind(),
            m.value_kind(),
            m.entries()
                .iter()
                .map(|(k, v)| Ok((strip_meta(k)?, strip_meta(v)?)))
                .collect::<Result<_, Error>>()?,
        )?),
    })
}

fn strip_struct<M>(value: &values::Struct<M>) -> Result<values::Struct, Error> {
    OwnedNode::from(value).to_struct()
}

fn strip_container<M>(value: &values::Container<M>) -> Result<values::Container, Error> {
    values::Container::new(
        value.item_kind(),
        value
            .items()
            .iter()
            .map(strip_meta)
            .collect::<Result<_, Error>>()?,
    )
}
