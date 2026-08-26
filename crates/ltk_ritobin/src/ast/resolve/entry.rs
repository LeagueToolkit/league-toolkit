use ltk_hash::BinHash;
use ltk_meta::{property::values, traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::{
        build::{BuildCtx, ChildrenExt as _},
        coerce::CanCoerce as _,
        diagnostics::{
            Diagnostic::{
                self, CustomSpan, InvalidHash, MissingTree, MissingType, QuotedPropertyName,
                TypeMismatch,
            },
            MaybeSpanDiag,
        },
        hash::HashedLiteral,
        resolve::literals::{self},
        AstValue,
    },
    cst::Kind,
    parse::{Span, Token, TokenKind},
    Node, RitoType, Spanned,
};

pub struct RawEntry {
    pub key: AstValue,
    pub type_span: Option<Span>,
    pub value: AstValue,
}

impl<'a> BuildCtx<'a> {
    pub fn resolve_entry_key(
        &mut self,
        key_node: &Node,
        parent_value_kind: Option<RitoType>,
        parent_type_span: Option<Span>,
    ) -> Result<AstValue, Diagnostic> {
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
            }) => AstValue::String(Spanned::new(*span, self.text[span].into())),
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
                AstValue::from(values::String::new_with_meta(
                    self.text[Span::new(span.start + 1, span.end - 1)].into(),
                    *span,
                ))
            }
            Some(Token {
                kind: TokenKind::HexLit,
                span,
            }) => AstValue::Hash(literals::eval_hash(self.text, *span)?),
            Some(token) => literals::eval(
                self.text,
                token,
                parent_value_kind
                    .and_then(|k| k.subtypes[0])
                    .map(RitoType::simple),
                parent_type_span,
            )?
            .ok_or(CustomSpan("Unrecognised literal", key_node.span))?,
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

        let kind_node = children.find_tree(self.cst, Kind::TypeExpr);
        let kind_span = kind_node.map(|k| k.span);
        let kind = kind_node.map(|t| self.resolve_type_expr(t)).transpose()?;

        let value_node = children
            .find_tree(self.cst, Kind::EntryValue)
            .ok_or(MissingTree(Kind::EntryValue))?;
        let value_span = value_node.span;

        if let Some(parent) = parent_value_kind.as_ref() {
            if let Some((kind, kind_span)) = kind.as_ref().zip(kind_span) {
                if !parent.can_coerce(*kind) {
                    self.push(
                        TypeMismatch {
                            span: kind_span,
                            expected: *parent,
                            expected_span: parent_type_span,
                            got: (*kind).into(),
                        }
                        .unwrap(),
                    );
                    return Ok(RawEntry {
                        key,
                        type_span: parent_type_span,
                        value: AstValue::default_for(*parent, value_span),
                    });
                }
            }
        }

        let kind = kind.or(parent_value_kind);
        let type_span = kind_span.or(parent_type_span);

        let resolved_val = match self.resolve_value(value_node, kind, type_span) {
            Ok(v) => v,
            Err(e) => match kind {
                Some(kind) => {
                    self.push(e.default_span(entry.span));
                    Some(AstValue::default_for(kind, value_span))
                }
                None => return Err(e.into()),
            },
        };

        let resolved_val = resolved_val.map(|value| match kind {
            Some(kind) if value.kind() == kind.base => value,
            Some(kind) => value.clone().coerce_to(kind.base).unwrap_or(value),
            None => value,
        });

        let value = match (kind, resolved_val) {
            (None, Some(value)) => value,
            (None, None) => return Err(MissingType(key.span()).into()),
            (Some(kind), Some(value)) => match value.kind() == kind.base {
                true => value,
                false => {
                    return Err(TypeMismatch {
                        span: value.span(),
                        expected: kind,
                        expected_span: kind_span,
                        got: value.rito_type().into(),
                    }
                    .into())
                }
            },
            (Some(kind), None) => AstValue::default_for(kind, value_span),
        };

        Ok(RawEntry {
            key,
            value,
            type_span,
        })
    }
}
