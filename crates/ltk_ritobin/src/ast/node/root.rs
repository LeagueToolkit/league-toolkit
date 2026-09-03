use std::{convert::Infallible, str::FromStr};

use crate::{
    ast::{
        node::{roots::Roots, TypeExpr},
        RootEntry, Value,
    },
    parse::Span,
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
    pub idx: usize,
    pub name: Spanned<RootKind>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<RootValue>,
}

impl Root {
    /// Get the span of the whole root property
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        self.name.span.cover(
            self.value
                .as_ref()
                .and_then(|v| v.as_value())
                .map(|v| v.span())
                .unwrap_or(self.type_expr.span),
        )
    }

    pub fn resolve_span(&self, roots: &Roots) -> Span {
        self.name.span.cover(
            match self.value.as_ref() {
                Some(RootValue::Value(value)) => Some(value.span()),
                Some(RootValue::Taken) => match *self.name {
                    RootKind::Entries => roots
                        .entries
                        .as_ref()
                        .and_then(|e| e.value.first())
                        .map(|v| v.span()),
                    _ => None,
                },
                None => todo!(),
            }
            .unwrap_or(self.type_expr.span),
        )
    }
}

#[derive(Debug, Clone)]
pub enum RootValue {
    Value(Value),
    /// Value was taken to resolve a known root.
    Taken,
}

impl RootValue {
    pub fn as_value(&self) -> Option<&Value> {
        match self {
            RootValue::Value(v) => Some(v),
            _ => None,
        }
    }
}
