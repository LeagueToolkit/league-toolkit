use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{self},
            MaybeSpanDiag,
        },
        Value,
    },
    cst::Kind,
    parse::{Span, Token},
    Node, RitoType,
};

impl<'a> Builder<'a> {
    pub(crate) fn resolve_literal(
        &mut self,
        text: &str,
        token: &Token,
        kind_hint: Option<RitoType>,
        kind_hint_span: Option<Span>,
    ) -> Value {
        match Value::eval(text, token, kind_hint, kind_hint_span) {
            Ok(value) => value,
            Err(e) => {
                self.push(e.default_span(token.span));
                kind_hint
                    .map(|k| Value::Unresolved {
                        span: token.span,
                        kind: k.base,
                    })
                    .unwrap_or(Value::Unknown(token.span))
            }
        }
    }
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
                Ok(self.resolve_literal(self.text, token, hint, hint_span))
            }
            Kind::ErrorTree => Ok(Value::Unknown(node.span)),
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
