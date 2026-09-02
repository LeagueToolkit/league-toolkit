//! The owned tree as the walk sees it: `&PropertyValueEnum` and [`OwnedNode`].

use std::{fmt, iter::Enumerate, slice};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use super::{
    tree::{sealed::Sealed, Child, Leaf, TreeKind as _, TreeNode, TreeValue},
    Error,
};
use crate::{property::values, property::Kind, BinObject, PropertyValueEnum};

/// The owned tree's node: a class hash and a borrowed property map.
///
/// [`BinObject`] and [`values::Struct`] both view as one, through `From`.
#[derive(Clone, Copy)]
pub struct OwnedNode<'a> {
    class_hash: BinHash,
    properties: &'a IndexMap<BinHash, PropertyValueEnum>,
}

impl<'a> OwnedNode<'a> {
    /// A node over `properties`, carrying `class_hash`.
    #[must_use]
    pub fn new(class_hash: BinHash, properties: &'a IndexMap<BinHash, PropertyValueEnum>) -> Self {
        Self {
            class_hash,
            properties,
        }
    }
}

impl<'a> From<&'a BinObject> for OwnedNode<'a> {
    fn from(object: &'a BinObject) -> Self {
        Self::new(object.class_hash, &object.properties)
    }
}

impl<'a> From<&'a values::Struct> for OwnedNode<'a> {
    fn from(value: &'a values::Struct) -> Self {
        Self::new(value.class_hash, &value.properties)
    }
}

impl fmt::Debug for OwnedNode<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedNode")
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.properties.len())
            .finish()
    }
}

impl Sealed for OwnedNode<'_> {}
impl Sealed for &PropertyValueEnum {}

/// The properties of an [`OwnedNode`], in order.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct OwnedProperties<'a> {
    inner: indexmap::map::Iter<'a, BinHash, PropertyValueEnum>,
}

impl<'a> Iterator for OwnedProperties<'a> {
    type Item = Result<(BinHash, &'a PropertyValueEnum), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|(field, value)| Ok((*field, value)))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for OwnedProperties<'_> {}
impl std::iter::FusedIterator for OwnedProperties<'_> {}

impl<'a> TreeNode<'a> for OwnedNode<'a> {
    type Value = &'a PropertyValueEnum;
    type Properties = OwnedProperties<'a>;

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
            properties: self.properties.clone(),
        })
    }
}

/// The values inside an owned container, optional or map.
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Debug)]
pub struct OwnedChildren<'a> {
    inner: OwnedChildrenInner<'a>,
}

#[derive(Debug)]
enum OwnedChildrenInner<'a> {
    Items(Enumerate<slice::Iter<'a, PropertyValueEnum>>),
    Entries(slice::Iter<'a, (PropertyValueEnum, PropertyValueEnum)>),
}

impl<'a> Iterator for OwnedChildren<'a> {
    type Item = Result<(Child<&'a PropertyValueEnum>, &'a PropertyValueEnum), Error>;

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

impl ExactSizeIterator for OwnedChildren<'_> {}
impl std::iter::FusedIterator for OwnedChildren<'_> {}

impl<'a> TreeValue<'a> for &'a PropertyValueEnum {
    type Node = OwnedNode<'a>;
    type Children = OwnedChildren<'a>;

    fn kind(&self) -> Kind {
        PropertyValueEnum::kind(self)
    }

    fn holds_node(&self) -> Result<bool, Error> {
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
        Ok((*self).clone())
    }
}
