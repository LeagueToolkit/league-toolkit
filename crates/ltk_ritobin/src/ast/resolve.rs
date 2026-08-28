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
        Value,
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
    ) -> Result<Value, Diagnostic> {
        self.resolve_value(node, Some(RitoType::simple(expected)), hint_span)
    }
}
