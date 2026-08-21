mod block_value;
mod class;
mod entry;
mod listlikes;
mod type_expr;
mod value;

use ltk_meta::PropertyKind;

use crate::{
    ast::{build::BuildCtx, AstValue},
    parse::Span,
    typecheck::diagnostics::{
        Diagnostic::{self, *},
        RitoTypeOrVirtual,
    },
    Node, RitoType,
};

impl<'a> BuildCtx<'a> {
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
