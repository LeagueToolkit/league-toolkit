pub mod builder;
pub mod diagnostics;
pub mod hash;
pub mod node;
pub mod query;
pub mod resolve;
pub mod visitor;

mod to_bin;

#[cfg(test)]
mod tests;

use std::{fmt, str::FromStr};

pub use crate::Spanned;
use indexmap::IndexMap;
use ltk_meta::PropertyKind;
pub use node::{Object, Property, RootEntry, Value};
use smallvec::SmallVec;
pub use to_bin::PartialBin;

use crate::{
    ast::{diagnostics::DiagnosticWithSpan, node::TypeExpr},
    rito, Cst, RitoType,
};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone)]
pub struct Ast {
    pub roots: Roots,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl Ast {
    pub fn root_entries(&self) -> impl Iterator<Item = &RootEntry> {
        self.roots.entries.iter().flat_map(|e| e.value.iter())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    Prop,
    Patch,
}

#[derive(Debug, Clone)]
/// Something that was resolved from a value, and the value that something came from.
pub struct FromValue<T> {
    pub value: Value,
    pub resolved: T,
}

pub type VersionRoot = KnownRoot<u32>;
pub type FileTypeRoot = KnownRoot<FileKind>;
pub type LinkedRoot = KnownRoot<Vec<String>>;
pub type EntriesRoot = KnownRoot<Vec<RootEntry>>;

#[derive(Debug, Clone, Copy)]
pub struct KnownRoot<V> {
    idx: usize,
    value: V,
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

#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub file_type: Option<FileTypeRoot>,
    pub version: Option<VersionRoot>,
    pub linked: Option<LinkedRoot>,
    pub entries: Option<EntriesRoot>,

    /// Ordered list of all top level roots
    pub all: Vec<Root>,
}

impl Roots {
    pub fn new(roots: impl IntoIterator<Item = Root>) -> Self {
        Self {
            all: roots.into_iter().collect(),
            ..Default::default()
        }
    }
}

impl<'a> IntoIterator for &'a Roots {
    type Item = &'a Root;

    type IntoIter = core::slice::Iter<'a, Root>;

    fn into_iter(self) -> Self::IntoIter {
        self.all.iter()
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

/// One of the four entries every ritobin file has at its root, or [`Self::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum RootEntryKind {
    #[default]
    Unknown,
    Version,
    Type,
    Linked,
    Entries,
}

impl RootEntryKind {
    /// The key this root entry uses in a ritobin file.
    /// [`Self::Unknown`] is not an actual root entry,
    /// but is written as `"unknown"` in this method.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Version => "version",
            Self::Linked => "linked",
            Self::Entries => "entries",
            Self::Unknown => "unknown",
        }
    }

    /// What type this kind of root expects
    pub fn expected_type(&self) -> Option<RitoType> {
        Some(match self {
            RootEntryKind::Unknown => return None,
            RootEntryKind::Version => rito!(U32),
            RootEntryKind::Type => rito!(String),
            RootEntryKind::Linked => rito!(Container[String]),
            RootEntryKind::Entries => rito!(Map[Hash, Embedded]),
        })
    }

    pub fn from_value(value: &Value) -> Self {
        let Value::String(string) = value else {
            return Self::Unknown;
        };

        let value = string.value.as_str();
        value.parse().unwrap_or(Self::Unknown)
    }
}

impl fmt::Display for RootEntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for RootEntryKind {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        Ok(match s {
            "type" => Self::Type,
            "version" => Self::Version,
            "linked" => Self::Linked,
            "entries" => Self::Entries,
            _ => return Err(()),
        })
    }
}

impl Cst {
    pub fn build_ast(&self, text: &str) -> crate::ast::Ast {
        crate::ast::Ast::from_cst(self, text)
    }
}
