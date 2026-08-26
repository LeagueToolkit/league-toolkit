use std::iter::once;

use crate::{
    ast::{
        build::{Ast, AstObject},
        query::{
            nodes::{DetailedNode, Node},
            path_iter::{AstFinePathIter, AstPathIter},
        },
        AstProperty, AstStruct, AstValue,
    },
    parse::Span,
};

mod detail;
pub mod nodes;
pub mod path_iter;

pub use detail::*;

impl AstObject {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = Node<'a>> {
        once(Node::Struct(&self.object))
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = DetailedNode<'a>> {
        [
            DetailedNode::Object(self, AstObjectDetail::PathHash),
            DetailedNode::Struct(&self.object, AstStructDetail::Node),
            DetailedNode::Object(self, AstObjectDetail::Trivia),
        ]
        .into_iter()
    }
}
impl AstStruct {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = Node<'a>> {
        self.properties.iter().map(Node::Property)
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = DetailedNode<'a>> {
        once(DetailedNode::Struct(self, AstStructDetail::ClassHash))
            .chain(
                self.properties
                    .iter()
                    .map(|v| DetailedNode::Property(v, AstPropertyDetail::Node)),
            )
            .chain(once(DetailedNode::Struct(self, AstStructDetail::Trivia)))
    }
}
impl AstProperty {
    pub fn children<'a>(&'a self) -> impl Iterator<Item = Node<'a>> {
        once(Node::Value(&self.value))
    }
    pub fn detailed_children<'a>(&'a self) -> impl Iterator<Item = DetailedNode<'a>> {
        [
            DetailedNode::Property(self, AstPropertyDetail::Name),
            DetailedNode::Property(self, AstPropertyDetail::TypeExpr),
            DetailedNode::Value(&self.value),
            DetailedNode::Property(self, AstPropertyDetail::Trivia),
        ]
        .into_iter()
    }
}

impl<'a> Node<'a> {
    pub fn span(&self) -> Span {
        match self {
            // TODO: don't do this
            Node::Object(o) => Span::new(
                o.path_hash.span().start.min(o.object.span.start),
                o.object.span.end.max(o.path_hash.span().end),
            ),
            Node::Struct(s) => s.span,
            Node::Property(p) => p.span(),
            Node::Value(v) => v.span(),
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
        AstPathIter::from_node(*self, offset)
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
    /// The chain of nodes on the way to `offset`, outermost first.
    pub fn coarse_path_to(&self, offset: u32) -> AstPathIter<'_> {
        AstPathIter::from_ast(self, offset)
    }
    /// The chain of nodes on the way to `offset`, outermost first
    pub fn fine_path_to(&self, offset: u32) -> AstFinePathIter<'_> {
        AstFinePathIter::from_ast(self, offset)
    }

    /// The most specific node containing `offset`. See [`Self::path_to`] if you need the full path.
    pub fn coarse_find_node(&self, offset: u32) -> Option<Node<'_>> {
        self.coarse_path_to(offset).last()
    }
    pub fn fine_find_node(&self, offset: u32) -> Option<DetailedNode<'_>> {
        self.fine_path_to(offset).last()
    }
}
