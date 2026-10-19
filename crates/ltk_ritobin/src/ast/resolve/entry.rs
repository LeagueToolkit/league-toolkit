use ltk_meta::PropertyKind;

use crate::{ast::diagnostics::RitoTypeOrVirtual, span::Span};
use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{self, MissingTree, MissingType, QuotedPropertyName},
            MaybeSpanDiag, TypeMismatch,
        },
        node::TypeExpr,
        resolve::literals::ExpectedType,
        Value,
    },
    cst::{ChildrenExt as _, Kind},
    parse::{Token, TokenKind},
    Node, RitoType, Spanned, SpannedExt,
};

pub struct RawEntry {
    pub key: Value,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

impl<'a> Builder<'a> {
    pub fn resolve_entry_key(
        &mut self,
        key_node: &Node,
        parent_value_kind: Option<RitoType>,
        parent_type_span: Option<Span>,
    ) -> Result<Value, Diagnostic> {
        use Diagnostic as D;
        let node = key_node
            .children
            .get(self.cst)
            .first()
            .ok_or(D::CustomSpan(
                "[resolve_entry_key] no children",
                key_node.span,
            ))?;

        let key_kind = parent_value_kind.and_then(|k| k.subtype(0));
        if let Some(block) = node
            .tree(self.cst)
            .filter(|tree| tree.kind == Kind::BlockKey)
        {
            let key_kind = key_kind.ok_or(D::CustomSpan(
                "[resolve_entry_key] no block key type",
                key_node.span,
            ))?;
            return self
                .resolve_block_value(block, key_kind, parent_type_span)
                .map_err(|e| e.diagnostic);
        }

        Ok(match node.token(self.cst) {
            Some(Token {
                kind: TokenKind::Name,
                span,
            }) => Value::String(Spanned::new(*span, self.text[span].into())),
            Some(Token {
                kind: TokenKind::String,
                span,
            }) => {
                if let Some(parent) = parent_value_kind
                    .filter(|p| matches!(p.base(), PropertyKind::Struct | PropertyKind::Embedded))
                {
                    self.push(
                        QuotedPropertyName {
                            span: *span,
                            parent,
                        }
                        .unwrap(),
                    );
                }
                let text =
                    crate::escaping::unescape(&self.text[Span::new(span.start + 1, span.end - 1)])
                        .map_err(|e| e.into_diagnostic(*span))?;
                Value::String(text.with_span(*span))
            }
            Some(Token {
                kind: TokenKind::HexLit,
                span,
            }) => Value::eval_unknown_hash(self.text, *span)?,
            Some(token) => self.resolve_literal(
                self.text,
                token,
                parent_value_kind
                    .and_then(|k| k.subtype(0))
                    .map(ExpectedType::from),
                parent_type_span,
            ),
            None => {
                return Err(D::CustomSpan(
                    "[resolve_entry_key] node not a token",
                    key_node.span,
                ))
            }
        })
    }

    /// Resolves an `Entry` node
    pub fn resolve_entry(
        &mut self,
        entry: &Node,
        parent_value_kind: Option<RitoType>,
        parent_type_span: Option<Span>,
    ) -> Result<RawEntry, MaybeSpanDiag> {
        let children = entry.children.get(self.cst);
        let key_node = children
            .find_tree(self.cst, Kind::EntryKey)
            .ok_or(MissingTree(Kind::EntryKey))?;
        let key = self.resolve_entry_key(key_node, parent_value_kind, parent_type_span)?;

        let parent_value_kind = parent_value_kind.and_then(|p| p.value_subtype());

        let type_expr_node = children.find_tree(self.cst, Kind::TypeExpr);
        let type_expr_span = type_expr_node.map(|k| k.span);
        let type_expr = type_expr_node.and_then(|t| self.resolve_type_expr(t));

        let value_node = children
            .find_tree(self.cst, Kind::EntryValue)
            .ok_or(MissingTree(Kind::EntryValue))?;

        let desired_kind = type_expr.or(parent_value_kind.map(TypeExpr::from));
        let type_span = type_expr_span.or(parent_type_span);

        let value = match self.resolve_value(
            value_node,
            desired_kind
                .and_then(|k| k.as_resolved())
                .map(ExpectedType::from),
            type_span,
        ) {
            Ok(v) => Some(v),
            Err(e) => {
                if !matches!(desired_kind, Some(TypeExpr::Unresolved)) {
                    // don't report the value error if the type expr was unresolved - we have
                    // bigger fish to fry, so reporting this error isn't needed
                    self.push(e.default_span(entry.span));
                }
                None
            }
        };

        let value = value.map(|value| match desired_kind {
            Some(TypeExpr::Resolved(kind)) => {
                let coerce = value.try_coerce_to(kind);
                match coerce {
                    Ok(value) => value,
                    Err(value) => value,
                }
            }
            _ => value,
        });

        match (desired_kind, value.as_ref()) {
            (Some(TypeExpr::Resolved(kind)), Some(value)) => {
                // a value doesn't preserve hash algo, so erase it here.
                // coercion makes sure we have the declared width.
                if value.rito_type().is_some_and(|k| {
                    !k.with_default_hash_types()
                        .equivalent_to(&kind.with_default_hash_types())
                }) {
                    self.push(Diagnostic::unwrap(
                        TypeMismatch {
                            span: value.span(),
                            expected: kind.into(),
                            expected_span: type_expr_span,
                            got: value.rito_type().into(),
                        }
                        .into(),
                    ))
                }
            }
            (None, None) => {
                self.push(MissingType(key.span()).unwrap());
            }
            // only report missing value if the type expression resolved properly (bigger fish)
            (Some(TypeExpr::Resolved(kind)), None) => {
                self.push(
                    Diagnostic::MissingEntryValue {
                        key_span: key_node.span,
                        expected: type_expr_span.map(|span| Spanned::new(span, kind)),
                    }
                    .unwrap(),
                );
            }
            _ => {}
        };

        Ok(RawEntry {
            key,
            value,
            type_expr: type_expr.with_span(
                (match type_expr_span {
                    Some(span) if !span.is_empty() => Some(span),
                    _ => None,
                })
                .unwrap_or_else(|| {
                    let colon = entry
                        .children
                        .find_token(self.cst, TokenKind::Colon)
                        .map(|t| t.span);
                    let eq = entry
                        .children
                        .find_token(self.cst, TokenKind::Eq)
                        .map(|t| t.span);

                    match (colon, eq) {
                        (None, None) => Span::new(key_node.span.end, entry.span.end),
                        (None, Some(eq)) => Span::new(key_node.span.start, eq.start),
                        (Some(colon), None) => Span::new(colon.end, entry.span.end),
                        (Some(colon), Some(eq)) => Span::new(colon.end, eq.start),
                    }
                }),
            ),
        })
    }
}
