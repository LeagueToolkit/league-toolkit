use ltk_hash::BinHash;

use crate::{
    ast::{
        hash::HashedLiteral,
        node::{NodeExt, NodeKind},
        query::{AstObjectDetail, AstPropertyDetail, AstStructDetail, NodeDetail},
        Object, Property, RootEntry, Value,
    },
    parse::Span,
};

/// A reference to a node in an [`Ast`].
#[derive(Debug, Clone, Copy)]
pub enum NodeRef<'a> {
    Object(&'a RootEntry),
    Struct(&'a Object),
    Property(&'a Property),
    Value(&'a Value),
}

/// A detailed reference to a node in an [`Ast`], down to the field level.
#[derive(Debug, Clone)]
pub enum SubNodeRef<'a> {
    Object(&'a RootEntry, AstObjectDetail),
    Struct(&'a Object, AstStructDetail),
    Property(&'a Property, AstPropertyDetail),
    Value(&'a Value),
}

impl NodeExt for NodeRef<'_> {
    fn kind(&self) -> NodeKind {
        match self {
            NodeRef::Object(_) => NodeKind::Object,
            NodeRef::Struct(_) => NodeKind::Struct,
            NodeRef::Property(_) => NodeKind::Property,
            NodeRef::Value(_) => NodeKind::Value,
        }
    }

    fn class_hash(&self) -> Option<HashedLiteral<BinHash>> {
        match self {
            NodeRef::Object(o) => Some(o.object.class_hash),
            NodeRef::Struct(s) => Some(s.class_hash),
            NodeRef::Property(_) | NodeRef::Value(_) => None,
        }
    }
}

impl NodeExt for SubNodeRef<'_> {
    #[inline(always)]
    fn kind(&self) -> NodeKind {
        match self {
            SubNodeRef::Object(_, _) => NodeKind::Object,
            SubNodeRef::Struct(_, _) => NodeKind::Struct,
            SubNodeRef::Property(_, _) => NodeKind::Property,
            SubNodeRef::Value(_) => NodeKind::Value,
        }
    }

    #[inline(always)]
    fn class_hash(&self) -> Option<HashedLiteral<BinHash>> {
        match self {
            Self::Object(o, _) => Some(o.object.class_hash),
            Self::Struct(s, _) => Some(s.class_hash),
            Self::Property(_, _) | Self::Value(_) => None,
        }
    }
}

impl<'a> SubNodeRef<'a> {
    #[inline(always)]
    #[must_use]
    pub fn detail(&self) -> NodeDetail {
        match self {
            SubNodeRef::Object(_, d) => (*d).into(),
            SubNodeRef::Struct(_, d) => (*d).into(),
            SubNodeRef::Property(_, d) => (*d).into(),
            SubNodeRef::Value(_) => NodeDetail::Value,
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn trivia_from(node: &NodeRef<'a>) -> Self {
        match node {
            NodeRef::Object(v) => Self::Object(v, AstObjectDetail::Trivia),
            NodeRef::Struct(v) => Self::Struct(v, AstStructDetail::Trivia),
            NodeRef::Property(v) => Self::Property(v, AstPropertyDetail::Trivia),
            NodeRef::Value(v) => Self::Value(v),
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        Some(match self {
            SubNodeRef::Object(v, f) => match f {
                AstObjectDetail::Node | AstObjectDetail::Trivia => v.span(),
                AstObjectDetail::PathHash => v.path_hash.span(),
            },
            SubNodeRef::Struct(v, f) => match f {
                AstStructDetail::Node | AstStructDetail::Trivia => v.span,
                AstStructDetail::ClassHash => v.class_hash.span(),
            },
            SubNodeRef::Property(v, f) => match f {
                AstPropertyDetail::Node | AstPropertyDetail::Trivia => v.span(),
                AstPropertyDetail::Name => v.name.span(),
                AstPropertyDetail::TypeExpr => v.type_span?,
            },
            SubNodeRef::Value(v) => v.span(),
        })
    }
}

impl<'a> From<&'a RootEntry> for NodeRef<'a> {
    fn from(value: &'a RootEntry) -> Self {
        Self::Object(value)
    }
}
impl<'a> From<&'a Object> for NodeRef<'a> {
    fn from(value: &'a Object) -> Self {
        Self::Struct(value)
    }
}
impl<'a> From<&'a Property> for NodeRef<'a> {
    fn from(value: &'a Property) -> Self {
        Self::Property(value)
    }
}
impl<'a> From<&'a Value> for NodeRef<'a> {
    fn from(value: &'a Value) -> Self {
        Self::Value(value)
    }
}

impl<'a> From<&'a RootEntry> for SubNodeRef<'a> {
    fn from(value: &'a RootEntry) -> Self {
        Self::Object(value, AstObjectDetail::Node)
    }
}
impl<'a> From<&'a Object> for SubNodeRef<'a> {
    fn from(value: &'a Object) -> Self {
        Self::Struct(value, AstStructDetail::Node)
    }
}
impl<'a> From<&'a Property> for SubNodeRef<'a> {
    fn from(value: &'a Property) -> Self {
        Self::Property(value, AstPropertyDetail::Node)
    }
}
impl<'a> From<&'a Value> for SubNodeRef<'a> {
    fn from(value: &'a Value) -> Self {
        Self::Value(value)
    }
}
impl<'a> From<NodeRef<'a>> for SubNodeRef<'a> {
    fn from(value: NodeRef<'a>) -> Self {
        match value {
            NodeRef::Object(v) => v.into(),
            NodeRef::Struct(v) => v.into(),
            NodeRef::Property(v) => v.into(),
            NodeRef::Value(v) => v.into(),
        }
    }
}
