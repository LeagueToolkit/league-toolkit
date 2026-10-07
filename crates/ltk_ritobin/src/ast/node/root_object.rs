use ltk_hash::BinHash;

use crate::ast::{hash::HashedLiteral, node::Object};
use crate::span::Span;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
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
