use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{traits::PropertyExt as _, PropertyKind, PropertyValueEnum};

use crate::{
    ast::{
        build::{BuildCtx, ChildrenExt as _},
        AstProperty, AstStruct, AstValue, Spanned,
    },
    cst::Kind,
    literals::{self, CoerceFrom as _},
    parse::{Span, Token, TokenKind},
    typecheck::diagnostics::{
        Diagnostic::{self, *},
        MaybeSpanDiag, RitoTypeOrVirtual,
    },
    Node, PropertyValueExt as _, RitoType, RitobinName as _,
};

impl<'a> BuildCtx<'a> {
    pub(crate) fn resolve_class_hash(&mut self, token: &Token) -> Result<BinHash, Diagnostic> {
        match token {
            Token {
                kind: TokenKind::Name,
                span,
            } => Ok(BinHash::hash_str(&self.text[span])),
            Token {
                kind: TokenKind::HexLit,
                span,
            } => match literals::eval_unknown_hash(self.text, *span)? {
                PropertyValueEnum::Hash(hash) => Ok(*hash),
                value => Err(TypeMismatch {
                    span: *value.meta(),
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
    ) -> Result<AstValue, Diagnostic> {
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

        let ast_struct = AstStruct {
            class_hash: Spanned::new(name_token.span, class_hash),
            span: class.span,
            properties,
        };

        Ok(match hint.base {
            PropertyKind::Struct => AstValue::Struct(ast_struct),
            PropertyKind::Embedded => AstValue::Embedded(ast_struct),
            _ => unreachable!(),
        })
    }
}
