use ltk_hash::{BinHash, Hash as _};

use crate::{
    ast::{
        builder::Builder,
        diagnostics::{Diagnostic as D, RitoTypeOrVirtual, TypeMismatch},
        hash::HashedLiteral,
        Object, Value,
    },
    cst::{ChildrenExt as _, Kind},
    parse::{Token, TokenKind},
    rito, Node, RitoType, ScalarType,
};

impl<'a> Builder<'a> {
    pub(crate) fn resolve_class_hash(
        &mut self,
        token: &Token,
    ) -> Result<HashedLiteral<BinHash>, D> {
        match token {
            Token {
                kind: TokenKind::Name,
                span,
            } => Ok(HashedLiteral::new(
                *span,
                crate::ast::hash::Originally::Name,
                BinHash::hash_str(&self.text[span]),
            )),
            Token {
                kind: TokenKind::HexLit,
                span,
            } => match Value::eval_unknown_hash(self.text, *span)? {
                Value::Hash(hash) => hash.try_as_bin_hash().ok_or(D::InvalidHash(hash.span())),
                value => Err(TypeMismatch {
                    span: value.span(),
                    expected: rito!(Hash).into(),
                    expected_span: None,
                    got: value.rito_type().into(),
                }
                .into()),
            },
            _ => Err(D::CustomSpan(
                "[resolve_class_hash] unexpected token kind",
                token.span,
            )),
        }
    }
    pub(crate) fn resolve_class(&mut self, class: &Node, hint: RitoType) -> Result<Value, D> {
        let children = class.children.get(self.cst);
        let Some(name_token) = children.first().and_then(|c| c.token(self.cst)) else {
            return Err(D::CustomSpan(
                "[resolve_class] first child missing/not a token",
                class.span,
            ));
        };
        let class_hash = self.resolve_class_hash(name_token)?;

        let wrap = match hint {
            RitoType::Scalar(ScalarType::Struct) => Value::Struct,
            RitoType::Scalar(ScalarType::Embedded) => Value::Embedded,
            _ => {
                return Err(TypeMismatch {
                    span: name_token.span,
                    expected: hint.into(),
                    expected_span: None,
                    got: RitoTypeOrVirtual::StructOrEmbedded,
                }
                .into())
            }
        };

        let properties = match children.find_tree(self.cst, Kind::Block) {
            Some(block) => self.resolve_body_properties(block, hint),
            None => Vec::new(),
        };

        let ast_struct = Object {
            class_hash,
            span: class.span,
            properties,
        };

        Ok(wrap(ast_struct))
    }
}
