use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{self, *},
            MaybeSpanDiag,
        },
        resolve::literals::{self},
        Value,
    },
    cst::Kind,
    parse::Span,
    Node, RitoType,
};

impl<'a> Builder<'a> {
    pub(crate) fn resolve_value(
        &mut self,
        wrapper: &Node,
        hint: Option<RitoType>,
        hint_span: Option<Span>,
    ) -> Result<Value, Diagnostic> {
        let Some(child) = wrapper.children.get(self.cst).first() else {
            return Err(Diagnostic::CustomSpan(
                "[resolve_value] node has no children",
                wrapper.span,
            ));
        };
        let Some(node) = child.tree(self.cst) else {
            return Err(Diagnostic::CustomSpan(
                "[resolve_value] first child is not a node",
                wrapper.span,
            ));
        };
        match node.kind {
            Kind::Class => {
                let Some(hint) = hint else {
                    return Err(Diagnostic::CustomSpan(
                        "Cannot resolve class block with no type hint",
                        node.span,
                    ));
                };
                self.resolve_class(node, hint)
            }
            Kind::Block => {
                let Some(hint) = hint else {
                    return Err(Diagnostic::CustomSpan(
                        "Cannot resolve block with no type hint",
                        node.span,
                    ));
                };
                self.resolve_block_value(node, hint, hint_span)
                    .map_err(|e| e.fallback(node.span).diagnostic)
            }
            Kind::Literal => {
                let Some(token_child) = node.children.get(self.cst).first() else {
                    return Err(Diagnostic::CustomSpan(
                        "[resolve_value] literal node has no children",
                        wrapper.span,
                    ));
                };
                let Some(token) = token_child.token(self.cst) else {
                    return Err(Diagnostic::CustomSpan(
                        "[resolve_value] literal node's first child is not a token",
                        wrapper.span,
                    ));
                };
                literals::eval(self.text, token, hint, hint_span).and_then(|v| {
                    v.ok_or(Diagnostic::CustomSpan(
                        "[resolve_value] literal token failed eval",
                        token.span,
                    ))
                })
            }
            Kind::ErrorTree => Err(Diagnostic::CustomSpan(
                "Cannot resolve value, syntax errors",
                node.span,
            )),
            kind => {
                eprintln!("cannot resolve {kind:?}");
                Err(Diagnostic::CustomSpan(
                    "[resolve_value] cannot resolve this node kind",
                    node.span,
                ))
            }
        }
    }

    pub(crate) fn resolve_list_item_block(
        &mut self,
        node: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Result<Value, MaybeSpanDiag> {
        self.resolve_block_value(node, hint, hint_span)
    }
}
