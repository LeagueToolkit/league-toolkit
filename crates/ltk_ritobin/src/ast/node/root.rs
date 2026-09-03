use std::{convert::Infallible, str::FromStr};

use crate::{
    ast::{
        node::{roots::Roots, TypeExpr},
        RootEntry, Value,
    },
    Spanned,
};

mod kind;
pub use kind::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    Prop,
    Patch,
    Unknown,
}

impl FromStr for FileKind {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "PROP" => Self::Prop,
            "PTCH" => Self::Patch,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KnownRoot<V> {
    pub(crate) idx: usize,
    pub value: V,
}

impl<V> KnownRoot<V> {
    /// Get the original root that this information was derived from
    pub fn original<'a>(&self, roots: &'a Roots) -> &'a Root {
        &roots.all[self.idx]
    }

    pub fn into_inner(self) -> V {
        self.value
    }
}

#[derive(Debug, Clone)]
/// A root is a special-cased property (`key: type = value`), that exists at the top level of a
/// ritobin file.
pub struct Root {
    pub name: Spanned<RootEntryKind>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

#[derive(Debug, Clone)]
pub enum RootValue {
    Value(Value),
    Dependencies(Spanned<Vec<Value>>),
    Entries(Spanned<Vec<RootEntry>>),
}

impl RootValue {
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            RootValue::Value(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_entries(&self) -> Option<&Vec<RootEntry>> {
        match self {
            RootValue::Entries(v) => Some(v),
            _ => None,
        }
    }
    pub fn as_dependencies(&self) -> Option<&Vec<Value>> {
        match self {
            RootValue::Dependencies(v) => Some(v),
            _ => None,
        }
    }
}
