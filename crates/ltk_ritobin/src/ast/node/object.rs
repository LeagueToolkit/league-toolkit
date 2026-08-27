use ltk_hash::BinHash;

use crate::{
    ast::{hash::HashedLiteral, node::Object, Ptr},
    parse::Span,
};

#[derive(Debug, Clone)]
pub struct RootObject {
    pub path_hash: HashedLiteral<BinHash>,
    pub object: Ptr<Object>,
}

impl RootObject {
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        Span::new(self.path_hash.span().start, self.object.span.end)
    }
}
