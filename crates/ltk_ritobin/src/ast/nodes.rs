use ltk_hash::BinHash;

use crate::{ast::AstValue, parse::Span};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    pub fn new(span: Span, value: T) -> Self {
        Self { span, value }
    }
}

#[derive(Debug, Clone)]
pub struct AstStruct {
    pub class_hash: Spanned<BinHash>,
    /// The entire `ClassName { .. }` span
    pub span: Span,
    pub properties: Vec<AstProperty>,
}

#[derive(Debug, Clone)]
pub struct AstProperty {
    pub name: Spanned<BinHash>,
    pub type_span: Option<Span>,
    pub value: AstValue,
}

impl AstProperty {
    /// Get the span of the whole property
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        let value_span = self.value.span();
        Span::new(self.name.span.start, value_span.end.max(self.name.span.end))
    }
}
