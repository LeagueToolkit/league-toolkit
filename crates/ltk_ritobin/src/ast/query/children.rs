use std::iter::{empty, once};

use crate::ast::{
    node::{
        root::{Root, RootValue},
        NodeRef, SubNodeRef,
    },
    Object, Property, RootEntry, Value,
};

use super::*;

impl<'a> NodeRef<'a> {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'a>> + 'a> {
        match self {
            NodeRef::Root(r) => r.children(),
            NodeRef::RootEntry(o) => Box::new(std::iter::once(NodeRef::Object(&o.object))),
            NodeRef::Object(s) => Box::new(s.properties.iter().map(NodeRef::Property)),
            NodeRef::Property(p) => Box::new(p.value.as_ref().map(NodeRef::Value).into_iter()),
            NodeRef::Value(v) => v.children(),
        }
    }
}

impl Root {
    pub fn children<'a>(&'a self) -> Box<dyn Iterator<Item = NodeRef<'a>> + 'a> {
        match &self.value {
            Some(RootValue::Entries(e)) => Box::new(e.iter().map(NodeRef::RootEntry)),
            Some(RootValue::Value(value)) => Box::new(once(NodeRef::Value(value))),
            // roots w/ simple values aren't worth a dedicated node at this resolution
            None => Box::new(empty()),
        }
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        [
            SubNodeRef::Root(self, AstRootDetail::Name),
            SubNodeRef::Root(self, AstRootDetail::TypeExpr),
        ]
        .into_iter()
        .chain(self.children().map(SubNodeRef::from))
        .chain(once(SubNodeRef::Root(self, AstRootDetail::Trivia)))
    }
}

impl RootEntry {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = NodeRef<'a>> {
        once(NodeRef::Object(&self.object))
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        [
            SubNodeRef::RootEntry(self, AstRootEntryDetail::PathHash),
            SubNodeRef::Object(&self.object, AstObjectDetail::Node),
            SubNodeRef::RootEntry(self, AstRootEntryDetail::Trivia),
        ]
        .into_iter()
    }
}

impl Object {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = NodeRef<'a>> {
        self.properties.iter().map(NodeRef::Property)
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        once(SubNodeRef::Object(self, AstObjectDetail::ClassHash))
            .chain(
                self.properties
                    .iter()
                    .map(|v| SubNodeRef::Property(v, AstPropertyDetail::Node)),
            )
            .chain(once(SubNodeRef::Object(self, AstObjectDetail::Trivia)))
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
        .chain(self.value.as_ref().map(SubNodeRef::Value))
        .chain(once(SubNodeRef::Property(self, AstPropertyDetail::Trivia)))
    }
}

impl Value {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'_>> + '_> {
        match self {
            Value::Struct(s) | Value::Embedded(s) => Box::new(std::iter::once(NodeRef::Object(s))),
            Value::Container { items, .. } | Value::UnorderedContainer { items, .. } => {
                Box::new(items.iter().map(NodeRef::Value))
            }
            Value::Map { entries, .. } => {
                Box::new(entries.iter().flat_map(|(k, v)| {
                    once(NodeRef::Value(k)).chain(v.as_ref().map(NodeRef::Value))
                }))
            }
            Value::Optional {
                value: Some(inner), ..
            } => Box::new(std::iter::once(NodeRef::Value(inner))),
            _ => Box::new(std::iter::empty()),
        }
    }
}
