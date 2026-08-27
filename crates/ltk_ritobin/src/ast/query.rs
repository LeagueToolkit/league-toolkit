mod children;
mod detail;
mod nodes;

pub mod path;

pub use detail::*;

use crate::{ast::node::NodeRef, parse::Span};

impl<'a> NodeRef<'a> {
    pub fn span(&self) -> Span {
        match self {
            // TODO: don't do this
            NodeRef::Object(o) => Span::new(
                o.path_hash.span().start.min(o.object.span.start),
                o.object.span.end.max(o.path_hash.span().end),
            ),
            NodeRef::Struct(s) => s.span,
            NodeRef::Property(p) => p.span(),
            NodeRef::Value(v) => v.span(),
        }
    }
}
