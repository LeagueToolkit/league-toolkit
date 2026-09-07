use ltk_hash::BinHash;

use crate::{
    ast::{
        hash::HashedLiteral,
        node::{root::Root, NodeExt, NodeKind},
        query::{
            AstObjectDetail, AstPropertyDetail, AstRootDetail, AstRootEntryDetail, NodeDetail,
        },
        Object, Property, RootEntry, Value,
    },
    parse::Span,
};

/// A reference to a node in an [`Ast`].
#[derive(Debug, Clone, Copy)]
pub enum NodeRef<'a> {
    Root(&'a Root),
    RootEntry(&'a RootEntry),
    Object(&'a Object),
    Property(&'a Property),
    Value(&'a Value),
}

/// A detailed reference to a node in an [`Ast`], down to the field level.
#[derive(Debug, Clone, Copy)]
pub enum SubNodeRef<'a> {
    Root(&'a Root, AstRootDetail),
    RootEntry(&'a RootEntry, AstRootEntryDetail),
    Object(&'a Object, AstObjectDetail),
    Property(&'a Property, AstPropertyDetail),
    Value(&'a Value),
}

impl<'a> NodeRef<'a> {
    pub fn span(&self) -> Span {
        match self {
            NodeRef::Root(r) => r.span(),
            NodeRef::RootEntry(o) => o.span(),
            NodeRef::Object(s) => s.span,
            NodeRef::Property(p) => p.span(),
            NodeRef::Value(v) => v.span(),
        }
    }
}
impl<'a> SubNodeRef<'a> {
    #[inline(always)]
    #[must_use]
    pub fn detail(&self) -> NodeDetail {
        match self {
            SubNodeRef::Root(_, d) => (*d).into(),
            SubNodeRef::RootEntry(_, d) => (*d).into(),
            SubNodeRef::Object(_, d) => (*d).into(),
            SubNodeRef::Property(_, d) => (*d).into(),
            SubNodeRef::Value(_) => NodeDetail::Value,
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn trivia_from(node: &NodeRef<'a>) -> Self {
        match node {
            NodeRef::Root(r) => Self::Root(r, AstRootDetail::Trivia),
            NodeRef::RootEntry(v) => Self::RootEntry(v, AstRootEntryDetail::Trivia),
            NodeRef::Object(v) => Self::Object(v, AstObjectDetail::Trivia),
            NodeRef::Property(v) => Self::Property(v, AstPropertyDetail::Trivia),
            NodeRef::Value(v) => Self::Value(v),
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            SubNodeRef::Root(v, f) => match f {
                AstRootDetail::Node | AstRootDetail::Trivia => v.span(),
                AstRootDetail::Name => v.name.span,
                AstRootDetail::TypeExpr => v.type_expr.span,
            },
            SubNodeRef::RootEntry(v, f) => match f {
                AstRootEntryDetail::Node | AstRootEntryDetail::Trivia => v.span(),
                AstRootEntryDetail::PathHash => v.path_hash.span(),
            },
            SubNodeRef::Object(v, f) => match f {
                AstObjectDetail::Node | AstObjectDetail::Trivia => v.span,
                AstObjectDetail::ClassHash => v.class_hash.span(),
            },
            SubNodeRef::Property(v, f) => match f {
                AstPropertyDetail::Node | AstPropertyDetail::Trivia => v.span(),
                AstPropertyDetail::Name => v.name.span(),
                AstPropertyDetail::TypeExpr => v.type_expr.span,
            },
            SubNodeRef::Value(v) => v.span(),
        }
    }
}

impl NodeExt for NodeRef<'_> {
    fn kind(&self) -> NodeKind {
        match self {
            NodeRef::Root(..) => NodeKind::Root,
            NodeRef::RootEntry(_) => NodeKind::RootEntry,
            NodeRef::Object(_) => NodeKind::Object,
            NodeRef::Property(_) => NodeKind::Property,
            NodeRef::Value(_) => NodeKind::Value,
        }
    }

    fn class_hash(&self) -> Option<HashedLiteral<BinHash>> {
        match self {
            NodeRef::RootEntry(o) => Some(o.object.class_hash),
            NodeRef::Object(s) => Some(s.class_hash),
            NodeRef::Property(_) | NodeRef::Value(_) | NodeRef::Root(..) => None,
        }
    }
}

impl NodeExt for SubNodeRef<'_> {
    #[inline(always)]
    fn kind(&self) -> NodeKind {
        match self {
            SubNodeRef::Root(..) => NodeKind::Root,
            SubNodeRef::RootEntry(_, _) => NodeKind::RootEntry,
            SubNodeRef::Object(_, _) => NodeKind::Object,
            SubNodeRef::Property(_, _) => NodeKind::Property,
            SubNodeRef::Value(_) => NodeKind::Value,
        }
    }

    #[inline(always)]
    fn class_hash(&self) -> Option<HashedLiteral<BinHash>> {
        match self {
            Self::RootEntry(o, _) => Some(o.object.class_hash),
            Self::Object(s, _) => Some(s.class_hash),
            Self::Property(_, _) | Self::Value(_) | Self::Root(..) => None,
        }
    }
}

impl<'a> From<&'a RootEntry> for NodeRef<'a> {
    fn from(value: &'a RootEntry) -> Self {
        Self::RootEntry(value)
    }
}
impl<'a> From<&'a Object> for NodeRef<'a> {
    fn from(value: &'a Object) -> Self {
        Self::Object(value)
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
        Self::RootEntry(value, AstRootEntryDetail::Node)
    }
}
impl<'a> From<&'a Object> for SubNodeRef<'a> {
    fn from(value: &'a Object) -> Self {
        Self::Object(value, AstObjectDetail::Node)
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
            NodeRef::Root(r) => Self::Root(r, AstRootDetail::Node),
            NodeRef::RootEntry(v) => v.into(),
            NodeRef::Object(v) => v.into(),
            NodeRef::Property(v) => v.into(),
            NodeRef::Value(v) => v.into(),
        }
    }
}
