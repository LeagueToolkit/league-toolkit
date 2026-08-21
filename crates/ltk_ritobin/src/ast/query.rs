//! Span-containment lookup on an already-resolved [`Ast`] - how hover is served. See the
//! [`crate::ast`] module docs: because the whole file is always resolved up front, hover never
//! needs a separate on-demand resolution step, just a lookup on a structure that already exists.

use crate::ast::{
    build::{Ast, AstObject},
    nodes::{AstProperty, AstStruct, AstValue},
};

pub enum Located<'a> {
    Object(&'a AstObject),
    Property(&'a AstProperty),
    Value(&'a AstValue),
}

impl Ast {
    /// Finds the most specific node covering `offset`, descending from objects into properties
    /// into nested values. `None` if `offset` falls outside every object (e.g. in the root
    /// `type`/`version`/`linked` entries, or between objects).
    pub fn locate(&self, offset: u32) -> Option<Located<'_>> {
        let obj = self
            .objects
            .iter()
            .find(|o| o.object.span.contains(offset) || o.path_hash.span.contains(offset))?;
        Some(obj.object.locate(offset).unwrap_or(Located::Object(obj)))
    }
}

impl AstStruct {
    pub fn locate(&self, offset: u32) -> Option<Located<'_>> {
        let prop = self.properties.iter().find(|p| p.span().contains(offset))?;
        Some(prop.locate(offset).unwrap_or(Located::Property(prop)))
    }
}

impl AstProperty {
    pub fn locate(&self, offset: u32) -> Option<Located<'_>> {
        if self.name.span.contains(offset) {
            return None;
        }
        self.value.locate(offset)
    }
}

impl AstValue {
    pub fn locate(&self, offset: u32) -> Option<Located<'_>> {
        if !self.span().contains(offset) {
            return None;
        }
        match self {
            AstValue::Struct(s) | AstValue::Embedded(s) => {
                Some(s.locate(offset).unwrap_or(Located::Value(self)))
            }
            AstValue::Container { items, .. } | AstValue::UnorderedContainer { items, .. } => Some(
                items
                    .iter()
                    .find_map(|i| i.locate(offset))
                    .unwrap_or(Located::Value(self)),
            ),
            AstValue::Map { entries, .. } => Some(
                entries
                    .iter()
                    .find_map(|(k, val)| k.locate(offset).or_else(|| val.locate(offset)))
                    .unwrap_or(Located::Value(self)),
            ),
            AstValue::Optional {
                value: Some(inner), ..
            } => Some(inner.locate(offset).unwrap_or(Located::Value(self))),
            _ => Some(Located::Value(self)),
        }
    }
}
