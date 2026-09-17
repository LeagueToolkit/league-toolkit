//! The owned tree as the walk sees it: `&PropertyValueEnum` and [`NodeRef`].

use std::{fmt, iter::Enumerate, slice};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, ChildSegment, Leaf, TreeKind as _, TreeNode, TreeValue},
    Error,
};
use crate::{property::values, property::Kind, BinObject, PropertyValueEnum};

/// The owned tree's node: a class hash and a borrowed property map.
///
/// [`BinObject`] and [`values::Struct`] both view as one, through `From`.
pub struct NodeRef<'a> {
    class_hash: BinHash,
    properties: &'a IndexMap<BinHash, PropertyValueEnum>,
}

impl<'a> NodeRef<'a> {
    /// A node over `properties`, carrying `class_hash`.
    #[must_use]
    pub fn new(class_hash: BinHash, properties: &'a IndexMap<BinHash, PropertyValueEnum>) -> Self {
        Self {
            class_hash,
            properties,
        }
    }
}

impl<'a> From<&'a BinObject> for NodeRef<'a> {
    fn from(object: &'a BinObject) -> Self {
        Self::new(object.class_hash, &object.properties)
    }
}

impl<'a> From<&'a values::Struct> for NodeRef<'a> {
    fn from(value: &'a values::Struct) -> Self {
        Self::new(value.class_hash, &value.properties)
    }
}

// By hand rather than derived: a derived `Copy` would demand `M: Copy` for a borrow.
impl Clone for NodeRef<'_> {
    fn clone(&self) -> Self {
        *self
    }
}
impl Copy for NodeRef<'_> {}

impl fmt::Debug for NodeRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRef")
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.properties.len())
            .finish()
    }
}

impl Sealed for NodeRef<'_> {}
impl Sealed for &PropertyValueEnum {}

/// The properties of a [`NodeRef`], in order.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct PropertiesRef<'a> {
    inner: indexmap::map::Iter<'a, BinHash, PropertyValueEnum>,
}

impl<'a> Iterator for PropertiesRef<'a> {
    type Item = Result<(BinHash, &'a PropertyValueEnum), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(field, value)| Ok((*field, value)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for PropertiesRef<'_> {}
impl std::iter::FusedIterator for PropertiesRef<'_> {}

impl<'a> TreeNode<'a> for NodeRef<'a> {
    type Value = &'a PropertyValueEnum;
    type Properties = PropertiesRef<'a>;

    fn class_hash(&self) -> BinHash {
        self.class_hash
    }

    fn properties(&self) -> Self::Properties {
        PropertiesRef {
            inner: self.properties.iter(),
        }
    }

    fn property(&self, field: BinHash) -> Result<Option<Self::Value>, Error> {
        Ok(self.properties.get(&field))
    }

    fn to_struct(&self) -> Result<values::Struct, Error> {
        Ok(values::Struct {
            class_hash: self.class_hash,
            properties: self.properties.clone(),
        })
    }
}

/// The values inside an owned container, optional or map.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct ChildrenRef<'a> {
    inner: ChildrenRefInner<'a>,
}

#[derive(Debug)]
enum ChildrenRefInner<'a> {
    Items(Enumerate<slice::Iter<'a, PropertyValueEnum>>),
    Entries(slice::Iter<'a, (PropertyValueEnum, PropertyValueEnum)>),
}

impl<'a> Iterator for ChildrenRef<'a> {
    type Item = Result<(ChildSegment<&'a PropertyValueEnum>, &'a PropertyValueEnum), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            ChildrenRefInner::Items(items) => items
                .next()
                .map(|(index, value)| Ok((ChildSegment::Index(index), value))),
            ChildrenRefInner::Entries(entries) => entries
                .next()
                .map(|(key, value)| Ok((ChildSegment::Key(key), value))),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            ChildrenRefInner::Items(items) => items.size_hint(),
            ChildrenRefInner::Entries(entries) => entries.size_hint(),
        }
    }
}

impl ExactSizeIterator for ChildrenRef<'_> {}
impl std::iter::FusedIterator for ChildrenRef<'_> {}

impl<'a> TreeValue<'a> for &'a PropertyValueEnum {
    type Node = NodeRef<'a>;
    type Children = ChildrenRef<'a>;

    fn kind(&self) -> Kind {
        PropertyValueEnum::kind(self)
    }

    fn can_contain_node(&self) -> Result<bool, Error> {
        Ok(match self {
            PropertyValueEnum::Struct(s) => *s.class_hash != 0,
            PropertyValueEnum::Embedded(e) => *e.0.class_hash != 0,
            PropertyValueEnum::Container(c) => c.item_kind().is_node(),
            PropertyValueEnum::UnorderedContainer(c) => c.0.item_kind().is_node(),
            PropertyValueEnum::Optional(o) => o.item_kind().is_node(),
            PropertyValueEnum::Map(m) => m.value_kind().is_node(),
            _ => false,
        })
    }

    fn as_node(&self) -> Result<Option<Self::Node>, Error> {
        let node = match self {
            PropertyValueEnum::Struct(s) => s,
            PropertyValueEnum::Embedded(e) => &e.0,
            _ => return Ok(None),
        };
        Ok((*node.class_hash != 0).then(|| NodeRef::from(node)))
    }

    fn children(&self) -> Result<Self::Children, Error> {
        let inner = match self {
            PropertyValueEnum::Container(c) => {
                ChildrenRefInner::Items(c.items().iter().enumerate())
            }
            PropertyValueEnum::UnorderedContainer(c) => {
                ChildrenRefInner::Items(c.0.items().iter().enumerate())
            }
            PropertyValueEnum::Optional(o) => ChildrenRefInner::Items(
                o.value()
                    .map_or(&[][..], slice::from_ref)
                    .iter()
                    .enumerate(),
            ),
            PropertyValueEnum::Map(m) => ChildrenRefInner::Entries(m.entries().iter()),
            _ => ChildrenRefInner::Items([].iter().enumerate()),
        };
        Ok(ChildrenRef { inner })
    }

    fn as_leaf(&self) -> Result<Option<Leaf<'a>>, Error> {
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
        Ok((*self).clone())
    }
}
