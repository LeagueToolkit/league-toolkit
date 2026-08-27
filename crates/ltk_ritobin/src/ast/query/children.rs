use std::iter::once;

use crate::{
    ast::{
        query::{
            nodes::{NodeRef, SubNodeRef},
            path::iter::{AstFinePathIter, AstPathIter},
        },
        Ast, Object, Property, RootObject, Value,
    },
    parse::Span,
};

use super::*;

impl<'a> NodeRef<'a> {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'a>> + 'a> {
        match self {
            NodeRef::Object(o) => Box::new(std::iter::once(NodeRef::Struct(&o.object))),
            NodeRef::Struct(s) => Box::new(s.properties.iter().map(NodeRef::Property)),
            NodeRef::Property(p) => Box::new(std::iter::once(NodeRef::Value(&p.value))),
            NodeRef::Value(v) => v.children(),
        }
    }
}

impl RootObject {
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
        once(NodeRef::Value(&self.value))
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = SubNodeRef<'a>> {
        [
            SubNodeRef::Property(self, AstPropertyDetail::Name),
            SubNodeRef::Property(self, AstPropertyDetail::TypeExpr),
            SubNodeRef::Value(&self.value),
            SubNodeRef::Property(self, AstPropertyDetail::Trivia),
        ]
        .into_iter()
    }
}

impl Value {
    pub fn children(&self) -> Box<dyn Iterator<Item = NodeRef<'_>> + '_> {
        match self {
            Value::Struct(s) | Value::Embedded(s) => Box::new(std::iter::once(NodeRef::Struct(s))),
            Value::Container { items, .. } | Value::UnorderedContainer { items, .. } => {
                Box::new(items.iter().map(NodeRef::Value))
            }
            Value::Map { entries, .. } => Box::new(
                entries
                    .iter()
                    .flat_map(|(k, v)| [NodeRef::Value(k), NodeRef::Value(v)]),
            ),
            Value::Optional {
                value: Some(inner), ..
            } => Box::new(std::iter::once(NodeRef::Value(inner))),
            _ => Box::new(std::iter::empty()),
        }
    }
}
