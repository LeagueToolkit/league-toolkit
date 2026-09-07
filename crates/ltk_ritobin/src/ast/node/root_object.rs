use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::Object},
    parse::Span,
};

#[derive(Debug, Clone)]
pub struct RootEntry {
    pub path_hash: HashedLiteral<BinHash>,
    pub object: Object,
}

impl RootEntry {
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.path_hash.span().start, self.object.span.end)
    }
}
