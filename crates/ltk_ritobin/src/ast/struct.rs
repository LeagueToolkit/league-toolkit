use ltk_hash::BinHash;

use crate::{ast::AstProperty, parse::Span, Spanned};

#[derive(Debug, Clone)]
pub struct AstStruct {
    pub class_hash: Spanned<BinHash>,
    /// The entire `ClassName { .. }` span
    pub span: Span,
    pub properties: Vec<AstProperty>,
}

impl AstStruct {
    pub fn properties_span(&self) -> Option<Span> {
        self.properties
            .first()
            .zip(self.properties.last())
            .map(|(l, r)| Span::new(l.span().start, r.span().end))
    }
}
