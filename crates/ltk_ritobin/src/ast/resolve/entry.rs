use ltk_meta::{property::values, PropertyKind};

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{
            Diagnostic::{
                self, InvalidHash, MissingTree, MissingType, QuotedPropertyName, TypeMismatch,
            },
            MaybeSpanDiag,
        },
        node::TypeExpr,
        resolve::literals::{self},
        Value,
    },
    cst::{ChildrenExt as _, Kind},
    parse::{Span, Token, TokenKind},
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
        let token = key_node
            .children
            .get(self.cst)
            .first()
            .ok_or(InvalidHash(key_node.span))?
            .token(self.cst);

        Ok(match token {
            Some(Token {
                kind: TokenKind::Name,
                span,
            }) => Value::String(Spanned::new(*span, self.text[span].into())),
            Some(Token {
                kind: TokenKind::String,
                span,
            }) => {
                if let Some(parent) = parent_value_kind
                    .filter(|p| matches!(p.base, PropertyKind::Struct | PropertyKind::Embedded))
                {
                    self.push(
                        QuotedPropertyName {
                            span: *span,
                            parent,
                        }
                        .unwrap(),
                    );
                }
                Value::from(values::String::new_with_meta(
                    self.text[Span::new(span.start + 1, span.end - 1)].into(),
                    *span,
                ))
            }
            Some(Token {
                kind: TokenKind::HexLit,
                span,
            }) => Value::Hash(literals::eval_hash(self.text, *span)?),
            Some(token) => self.resolve_literal(
                self.text,
                token,
                parent_value_kind
                    .and_then(|k| k.subtypes[0])
                    .map(RitoType::simple),
                parent_type_span,
            ),
            None => return Err(InvalidHash(key_node.span)),
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

        let parent_value_kind = parent_value_kind
            .and_then(|p| p.value_subtype())
            .map(RitoType::simple);

        let type_expr_node = children.find_tree(self.cst, Kind::TypeExpr);
        let type_expr_span = type_expr_node.map(|k| k.span);
        let type_expr = type_expr_node.and_then(|t| self.resolve_type_expr(t));

        let value_node = children
            .find_tree(self.cst, Kind::EntryValue)
            .ok_or(MissingTree(Kind::EntryValue))?;

        let desired_kind = type_expr.or(parent_value_kind.map(|k| k.into()));
        let type_span = type_expr_span.or(parent_type_span);

        let value = match self.resolve_value(
            value_node,
            desired_kind.and_then(|k| k.as_resolved()),
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
            Some(TypeExpr::Resolved(kind)) => match value.try_coerce_to(kind.base) {
                Ok(value) => value,
                Err(value) => value,
            },
            _ => value,
        });

        match (desired_kind, value.as_ref()) {
            (Some(kind), Some(value)) => {
                if value.rito_type().is_some_and(|k| k != kind) {
                    self.push(
                        TypeMismatch {
                            span: value.span(),
                            expected: kind.into(),
                            expected_span: type_expr_span,
                            got: value.rito_type().into(),
                        }
                        .unwrap(),
                    )
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
