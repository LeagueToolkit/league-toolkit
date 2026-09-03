use ltk_hash::BinHash;

use crate::{
    ast::{diagnostics::RitoTypeOrVirtual, hash::HashedLiteral, node::Value},
    parse::Span,
    RitoType, Spanned,
};

#[derive(Debug, Clone)]
pub struct Property {
    pub name: HashedLiteral<BinHash>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

impl Property {
    /// Get the span of the whole property
    #[inline(always)]
    #[must_use]
    pub fn span(&self) -> Span {
        self.name.span().cover(
            self.value
                .as_ref()
                .map(|v| v.span())
                .unwrap_or(self.type_expr.span),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeExpr {
    Unresolved,
    Resolved(RitoType),
}

impl TypeExpr {
    pub fn as_resolved(self) -> Option<RitoType> {
        match self {
            TypeExpr::Unresolved => None,
            TypeExpr::Resolved(rito_type) => Some(rito_type),
        }
    }
}

impl PartialEq<RitoType> for TypeExpr {
    fn eq(&self, other: &RitoType) -> bool {
        match self {
            TypeExpr::Unresolved => false,
            TypeExpr::Resolved(rito_type) => rito_type.eq(other),
        }
    }
}
impl PartialEq<TypeExpr> for RitoType {
    fn eq(&self, other: &TypeExpr) -> bool {
        other == self
    }
}

impl From<RitoType> for TypeExpr {
    fn from(value: RitoType) -> Self {
        Self::Resolved(value)
    }
}

impl From<TypeExpr> for RitoTypeOrVirtual {
    fn from(value: TypeExpr) -> Self {
        match value {
            TypeExpr::Unresolved => Self::Unknown,
            TypeExpr::Resolved(rito_type) => rito_type.into(),
        }
    }
}
