use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::AstStruct, Ptr},
    parse::Span,
};

#[derive(Debug, Clone)]
pub struct AstObject {
    pub path_hash: HashedLiteral<BinHash>,
    pub object: Ptr<AstStruct>,
}

impl AstObject {
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.path_hash.span().start, self.object.span.end)
    }
}
