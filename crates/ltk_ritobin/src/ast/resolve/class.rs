use ltk_hash::{BinHash, Hash as _};
use ltk_meta::PropertyKind;

use crate::{
    ast::{
        builder::{Builder, ChildrenExt as _},
        diagnostics::{
            Diagnostic::{self, *},
            RitoTypeOrVirtual,
        },
        hash::HashedLiteral,
        Object, Value,
    },
    cst::Kind,
    parse::{Token, TokenKind},
    Node, PropertyValueExt as _, RitoType,
};

impl<'a> Builder<'a> {
    pub(crate) fn resolve_class_hash(
        &mut self,
        token: &Token,
    ) -> Result<HashedLiteral<BinHash>, Diagnostic> {
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
                Value::Hash(hash) => Ok(hash),
                value => Err(TypeMismatch {
                    span: value.span(),
                    expected: RitoType::simple(PropertyKind::Hash),
                    expected_span: None,
                    got: value.rito_type().into(),
                }),
            },
            _ => Err(InvalidHash(token.span)),
        }
    }
    pub(crate) fn resolve_class(
        &mut self,
        class: &Node,
        hint: RitoType,
    ) -> Result<Value, Diagnostic> {
        let children = class.children.get(self.cst);
        let Some(name_token) = children.first().and_then(|c| c.token(self.cst)) else {
            return Err(InvalidHash(class.span));
        };
        let class_hash = self.resolve_class_hash(name_token)?;

        if !matches!(hint.base, PropertyKind::Struct | PropertyKind::Embedded) {
            return Err(TypeMismatch {
                span: name_token.span,
                expected: RitoType::simple(hint.base),
                expected_span: None,
                got: RitoTypeOrVirtual::StructOrEmbedded,
            });
        }

        let properties = match children.find_tree(self.cst, Kind::Block) {
            Some(block) => self.resolve_body_properties(block, hint),
            None => Vec::new(),
        };

        let ast_struct = Object {
            class_hash,
            span: class.span,
            properties,
        };

        Ok(match hint.base {
            PropertyKind::Struct => Value::Struct(ast_struct),
            PropertyKind::Embedded => Value::Embedded(ast_struct),
            _ => unreachable!(),
        })
    }
}
