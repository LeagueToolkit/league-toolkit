mod block_value;
mod class;
mod entry;
mod listlikes;
pub mod literals;
mod type_expr;
mod value;

use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{self, *},
            RitoTypeOrVirtual,
        },
        AstValue,
    },
    parse::Span,
    Node, RitoType,
};

impl<'a> Builder<'a> {
    /// `node` is the `ListItem` wrapping the literal.
    pub(crate) fn resolve_numeric(
        &mut self,
        node: &Node,
        expected: PropertyKind,
        hint_span: Option<Span>,
    ) -> Result<AstValue, Diagnostic> {
        match self.resolve_value(node, Some(RitoType::simple(expected)), hint_span)? {
            Some(v) => Ok(v),
            None => Err(TypeMismatch {
                span: node.span,
                expected: RitoType::simple(expected),
                expected_span: hint_span,
                got: RitoTypeOrVirtual::numeric(),
            }),
        }
    }
}
