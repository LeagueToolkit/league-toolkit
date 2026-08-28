use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::Value},
    parse::Span,
};

#[derive(Debug, Clone)]
pub struct Property {
    pub name: HashedLiteral<BinHash>,
    // pub type_span: Option<Spanned<RitoType>>,
    pub type_span: Option<Span>,
    pub value: Option<Value>,
}

impl Property {
    /// Get the span of the whole property
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        match self.value.as_ref().map(|v| v.span()).or(self.type_span) {
            Some(s) => self.name.span().cover(s),
            None => self.name.span(),
        }
    }
}
