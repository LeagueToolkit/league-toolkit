use std::iter::once;

use crate::ast::{
    node::{NodeRef, SubNodeRef},
    Object, Property, RootEntry, Value,
};

use super::*;

impl<'a> NodeRef<'a> {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'a>> + 'a> {
        match self {
            NodeRef::Object(o) => Box::new(std::iter::once(NodeRef::Struct(&o.object))),
            NodeRef::Struct(s) => Box::new(s.properties.iter().map(NodeRef::Property)),
            NodeRef::Property(p) => Box::new(p.value.as_ref().map(NodeRef::Value).into_iter()),
            NodeRef::Value(v) => v.children(),
        }
    }
}

impl RootEntry {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = NodeRef<'a>> {
        once(NodeRef::Struct(&self.object))
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        [
            SubNodeRef::Object(self, AstObjectDetail::PathHash),
            SubNodeRef::Struct(&self.object, AstStructDetail::Node),
            SubNodeRef::Object(self, AstObjectDetail::Trivia),
        ]
        .into_iter()
    }
}

impl Object {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = NodeRef<'a>> {
        self.properties.iter().map(NodeRef::Property)
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        once(SubNodeRef::Struct(self, AstStructDetail::ClassHash))
            .chain(
                self.properties
                    .iter()
                    .map(|v| SubNodeRef::Property(v, AstPropertyDetail::Node)),
            )
            .chain(once(SubNodeRef::Struct(self, AstStructDetail::Trivia)))
    }
}

impl Property {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = NodeRef<'a>> {
        self.value.as_ref().map(NodeRef::Value).into_iter()
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        [
            SubNodeRef::Property(self, AstPropertyDetail::Name),
            SubNodeRef::Property(self, AstPropertyDetail::TypeExpr),
        ]
        .into_iter()
        .chain(self.value.as_ref().map(SubNodeRef::Value).into_iter())
        .chain(once(SubNodeRef::Property(self, AstPropertyDetail::Trivia)))
    }
}

impl Value {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'_>> + '_> {
        match self {
            Value::Struct(s) | Value::Embedded(s) => Box::new(std::iter::once(NodeRef::Struct(s))),
            Value::Container { items, .. } | Value::UnorderedContainer { items, .. } => {
                Box::new(items.iter().map(NodeRef::Value))
            }
            Value::Map { entries, .. } => Box::new(entries.iter().flat_map(|(k, v)| {
                once(NodeRef::Value(k)).chain(v.as_ref().map(NodeRef::Value).into_iter())
            })),
            Value::Optional {
                value: Some(inner), ..
            } => Box::new(std::iter::once(NodeRef::Value(inner))),
            _ => Box::new(std::iter::empty()),
        }
    }
}
