use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::Property},
    parse::Span,
};

#[derive(Debug, Clone)]
pub struct Object {
    pub class_hash: HashedLiteral<BinHash>,
    /// The entire `ClassName { .. }` span
    pub span: Span,
    pub properties: Vec<Property>,
}

impl Object {
    pub fn properties_span(&self) -> Option<Span> {
        self.properties
            .first()
            .zip(self.properties.last())
            .map(|(l, r)| Span::new(l.span().start, r.span().end))
    }
}
