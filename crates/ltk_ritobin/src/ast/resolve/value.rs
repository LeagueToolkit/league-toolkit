use ltk_meta::{traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::{
        build::{BuildCtx, ChildrenExt as _},
        AstProperty, AstValue, Spanned,
    },
    cst::Kind,
    literals::{self, CoerceFrom as _},
    parse::{Span, TokenKind},
    typecheck::diagnostics::{
        Diagnostic::{self, *},
        MaybeSpanDiag, RitoTypeOrVirtual,
    },
    Node, PropertyValueExt as _, RitoType, RitobinName as _,
};

impl<'a> BuildCtx<'a> {
    pub(crate) fn resolve_value(
        &mut self,
        wrapper: &Node,
        hint: Option<RitoType>,
        hint_span: Option<Span>,
    ) -> Result<Option<AstValue>, Diagnostic> {
        let Some(child) = wrapper.children.get(self.cst).first() else {
            return Ok(None);
        };
        let Some(node) = child.tree(self.cst) else {
            return Ok(None);
        };
        match node.kind {
            Kind::Class => {
                let Some(hint) = hint else { return Ok(None) };
                self.resolve_class(node, hint).map(Some)
            }
            Kind::Block => {
                let Some(hint) = hint else { return Ok(None) };
                if matches!(hint.base, PropertyKind::Struct | PropertyKind::Embedded) {
                    return Err(MissingClassName {
                        span: node.open_brace_span(self.cst),
                        expected: hint,
                    });
                }
                self.resolve_block_value(node, hint, hint_span)
                    .map(Some)
                    .map_err(|e| e.fallback(node.span).diagnostic)
            }
            Kind::Literal => {
                let Some(token_child) = node.children.get(self.cst).first() else {
                    return Ok(None);
                };
                let Some(token) = token_child.token(self.cst) else {
                    return Ok(None);
                };
                literals::eval(self.text, token, hint, hint_span).map(|v| v.map(AstValue::from))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn resolve_list_item_block(
        &mut self,
        node: &Node,
        hint: RitoType,
        hint_span: Option<Span>,
    ) -> Result<AstValue, MaybeSpanDiag> {
        if matches!(hint.base, PropertyKind::Struct | PropertyKind::Embedded) {
            return Err(MissingClassName {
                span: node.open_brace_span(self.cst),
                expected: hint,
            }
            .into());
        }
        self.resolve_block_value(node, hint, hint_span)
    }
}
