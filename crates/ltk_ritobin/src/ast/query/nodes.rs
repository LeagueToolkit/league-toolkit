use ltk_hash::BinHash;

use crate::{
    ast::{
        query::{AstObjectDetail, AstPropertyDetail, AstStructDetail, NodeDetail},
        AstObject, AstProperty, AstStruct, AstValue,
    },
    parse::Span,
    Spanned,
};

pub trait NodeExt {
    #[must_use]
    fn kind(&self) -> NodeKind;

    /// This node's own class, if it's an object or struct.
    #[must_use]
    fn class_hash(&self) -> Option<Spanned<BinHash>>;
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum NodeKind {
    Object,
    Struct,
    Property,
    Value,
}

/// Any node in an [`Ast`].
#[derive(Debug, Clone, Copy)]
pub enum Node<'a> {
    Object(&'a AstObject),
    Struct(&'a AstStruct),
    Property(&'a AstProperty),
    Value(&'a AstValue),
}

#[derive(Debug, Clone)]
pub enum DetailedNode<'a> {
    Object(&'a AstObject, AstObjectDetail),
    Struct(&'a AstStruct, AstStructDetail),
    Property(&'a AstProperty, AstPropertyDetail),
    Value(&'a AstValue),
}

impl NodeExt for Node<'_> {
    fn kind(&self) -> NodeKind {
        match self {
            Node::Object(_) => NodeKind::Object,
            Node::Struct(_) => NodeKind::Struct,
            Node::Property(_) => NodeKind::Property,
            Node::Value(_) => NodeKind::Value,
        }
    }

    fn class_hash(&self) -> Option<Spanned<BinHash>> {
        match self {
            Node::Object(o) => Some(o.object.class_hash),
            Node::Struct(s) => Some(s.class_hash),
            Node::Property(_) | Node::Value(_) => None,
        }
    }
}

impl NodeExt for DetailedNode<'_> {
    #[inline(always)]
    fn kind(&self) -> NodeKind {
        match self {
            DetailedNode::Object(_, _) => NodeKind::Object,
            DetailedNode::Struct(_, _) => NodeKind::Struct,
            DetailedNode::Property(_, _) => NodeKind::Property,
            DetailedNode::Value(_) => NodeKind::Value,
        }
    }

    #[inline(always)]
    fn class_hash(&self) -> Option<Spanned<BinHash>> {
        match self {
            Self::Object(o, _) => Some(o.object.class_hash),
            Self::Struct(s, _) => Some(s.class_hash),
            Self::Property(_, _) | Self::Value(_) => None,
        }
    }
}

impl<'a> DetailedNode<'a> {
    #[inline(always)]
    #[must_use]
    pub fn detail(&self) -> NodeDetail {
        match self {
            DetailedNode::Object(_, d) => (*d).into(),
            DetailedNode::Struct(_, d) => (*d).into(),
            DetailedNode::Property(_, d) => (*d).into(),
            DetailedNode::Value(_) => NodeDetail::Value,
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn trivia_from(node: &Node<'a>) -> Self {
        match node {
            Node::Object(v) => Self::Object(v, AstObjectDetail::Trivia),
            Node::Struct(v) => Self::Struct(v, AstStructDetail::Trivia),
            Node::Property(v) => Self::Property(v, AstPropertyDetail::Trivia),
            Node::Value(v) => Self::Value(v),
        }
    }

    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Option<Span> {
        Some(match self {
            DetailedNode::Object(v, f) => match f {
                AstObjectDetail::Node | AstObjectDetail::Trivia => v.span(),
                AstObjectDetail::PathHash => v.path_hash.span,
            },
            DetailedNode::Struct(v, f) => match f {
                AstStructDetail::Node | AstStructDetail::Trivia => v.span,
                AstStructDetail::ClassHash => v.class_hash.span,
            },
            DetailedNode::Property(v, f) => match f {
                AstPropertyDetail::Node | AstPropertyDetail::Trivia => v.span(),
                AstPropertyDetail::Name => v.name.span,
                AstPropertyDetail::TypeExpr => v.type_span?,
            },
            DetailedNode::Value(v) => v.span(),
        })
    }
}

impl<'a> From<&'a AstObject> for Node<'a> {
    fn from(value: &'a AstObject) -> Self {
        Self::Object(value)
    }
}
impl<'a> From<&'a AstStruct> for Node<'a> {
    fn from(value: &'a AstStruct) -> Self {
        Self::Struct(value)
    }
}
impl<'a> From<&'a AstProperty> for Node<'a> {
    fn from(value: &'a AstProperty) -> Self {
        Self::Property(value)
    }
}
impl<'a> From<&'a AstValue> for Node<'a> {
    fn from(value: &'a AstValue) -> Self {
        Self::Value(value)
    }
}

impl<'a> From<&'a AstObject> for DetailedNode<'a> {
    fn from(value: &'a AstObject) -> Self {
        Self::Object(value, AstObjectDetail::Node)
    }
}
impl<'a> From<&'a AstStruct> for DetailedNode<'a> {
    fn from(value: &'a AstStruct) -> Self {
        Self::Struct(value, AstStructDetail::Node)
    }
}
impl<'a> From<&'a AstProperty> for DetailedNode<'a> {
    fn from(value: &'a AstProperty) -> Self {
        Self::Property(value, AstPropertyDetail::Node)
    }
}
impl<'a> From<&'a AstValue> for DetailedNode<'a> {
    fn from(value: &'a AstValue) -> Self {
        Self::Value(value)
    }
}
impl<'a> From<Node<'a>> for DetailedNode<'a> {
    fn from(value: Node<'a>) -> Self {
        match value {
            Node::Object(v) => v.into(),
            Node::Struct(v) => v.into(),
            Node::Property(v) => v.into(),
            Node::Value(v) => v.into(),
        }
    }
}
