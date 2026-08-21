use ltk_hash::BinHash;

use crate::{
    ast::{
        build::{Ast, AstObject},
        nodes::{AstProperty, AstStruct, Spanned},
        AstValue,
    },
    parse::Span,
};

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

impl<'a> Node<'a> {
    pub fn kind(&self) -> NodeKind {
        match self {
            Node::Object(_) => NodeKind::Object,
            Node::Struct(_) => NodeKind::Struct,
            Node::Property(_) => NodeKind::Property,
            Node::Value(_) => NodeKind::Value,
        }
    }
    pub fn span(&self) -> Span {
        match self {
            // TODO: don't do this
            Node::Object(o) => Span::new(
                o.path_hash.span.start.min(o.object.span.start),
                o.object.span.end.max(o.path_hash.span.end),
            ),
            Node::Struct(s) => s.span,
            Node::Property(p) => p.span(),
            Node::Value(v) => v.span(),
        }
    }

    /// This node's own class, if it's an object or struct.
    pub fn class_hash(&self) -> Option<Spanned<BinHash>> {
        match self {
            Node::Object(o) => Some(o.object.class_hash),
            Node::Struct(s) => Some(s.class_hash),
            Node::Property(_) | Node::Value(_) => None,
        }
    }

    fn children(&self) -> Box<dyn Iterator<Item = Node<'a>> + 'a> {
        match self {
            Node::Object(o) => Box::new(std::iter::once(Node::Struct(&o.object))),
            Node::Struct(s) => Box::new(s.properties.iter().map(Node::Property)),
            Node::Property(p) => Box::new(std::iter::once(Node::Value(&p.value))),
            Node::Value(v) => v.children(),
        }
    }

    /// The chain of nodes on the way to `offset`, including this node.
    pub fn path_to(&self, offset: u32) -> AstPathIter<'a> {
        AstPathIter {
            next: self.span().contains(offset).then_some(*self),
            offset,
        }
    }
}

/// Iterator of every [`Node`] on the way to a given offset, from the top level.
///
/// Use [`Ast::path_to`] to construct this iterator.
#[derive(Clone)]
pub struct AstPathIter<'a> {
    next: Option<Node<'a>>,
    offset: u32,
}

impl<'a> Iterator for AstPathIter<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Node<'a>> {
        let current = self.next.take()?;
        self.next = current.children().find(|c| c.span().contains(self.offset));
        Some(current)
    }
}

impl AstValue {
    fn children(&self) -> Box<dyn Iterator<Item = Node<'_>> + '_> {
        match self {
            AstValue::Struct(s) | AstValue::Embedded(s) => {
                Box::new(std::iter::once(Node::Struct(s)))
            }
            AstValue::Container { items, .. } | AstValue::UnorderedContainer { items, .. } => {
                Box::new(items.iter().map(Node::Value))
            }
            AstValue::Map { entries, .. } => Box::new(
                entries
                    .iter()
                    .flat_map(|(k, v)| [Node::Value(k), Node::Value(v)]),
            ),
            AstValue::Optional {
                value: Some(inner), ..
            } => Box::new(std::iter::once(Node::Value(inner))),
            _ => Box::new(std::iter::empty()),
        }
    }
}

impl Ast {
    /// The chain of nodes on the way to `offset`, outermost first
    pub fn path_to(&self, offset: u32) -> AstPathIter<'_> {
        let next = self
            .objects
            .iter()
            .find(|o| o.object.span.contains(offset) || o.path_hash.span.contains(offset))
            .map(Node::Object);
        AstPathIter { next, offset }
    }

    /// The most specific node containing `offset`. See [`Self::path_to`] if you need the full path.
    pub fn find_node(&self, offset: u32) -> Option<Node<'_>> {
        self.path_to(offset).last()
    }
}
