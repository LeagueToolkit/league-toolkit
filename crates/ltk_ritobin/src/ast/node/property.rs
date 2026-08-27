use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::AstValue},
    parse::Span,
    RitoType, Spanned,
};

#[derive(Debug, Clone)]
pub struct AstProperty {
    pub name: HashedLiteral<BinHash>,
    // pub type_span: Option<Spanned<RitoType>>,
    pub type_span: Option<Span>,
    pub value: AstValue,
}

impl AstProperty {
    /// Get the span of the whole property
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        let value_span = self.value.span();
        Span::new(
            self.name.span().start,
            value_span.end.max(self.name.span().end),
        )
    }
}
