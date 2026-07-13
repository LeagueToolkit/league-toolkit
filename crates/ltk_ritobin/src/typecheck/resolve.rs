use std::{borrow::Cow, fmt::Debug};

use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{self, visitor::VisitCtx, Kind, Node},
    parse::{Span, Token, TokenKind},
    typecheck::{
        diagnostics::{self, Diagnostic, MaybeSpanDiag, RitoTypeOrVirtual},
        ir::IrEntry,
    },
    Cst, PropertyValueExt as _, RitoType, RitobinName,
};

use super::{state::Ctx, trace::trace};

use diagnostics::Diagnostic::*;

trait TreeIterExt<'a>: Iterator {
    fn expect_tree(&mut self, cst: &'a Cst, kind: cst::Kind) -> Result<&'a Node, Diagnostic>;
    fn expect_token(&mut self, cst: &'a Cst, kind: TokenKind) -> Result<&'a Token, Diagnostic>;
}

impl<'a, I> TreeIterExt<'a> for I
where
    I: Iterator<Item = &'a cst::Child>,
{
    fn expect_tree(&mut self, cst: &'a Cst, kind: cst::Kind) -> Result<&'a Node, Diagnostic> {
        self.find_map(|c| c.tree(cst).filter(|t| t.kind == kind))
            .ok_or(MissingTree(kind))
    }
    fn expect_token(&mut self, cst: &'a Cst, kind: TokenKind) -> Result<&'a Token, Diagnostic> {
        self.find_map(|c| c.token(cst).filter(|t| t.kind == kind))
            .ok_or(MissingToken(kind))
    }
}

pub(crate) fn coerce_type<M: Debug + Default>(
    value: PropertyValueEnum<M>,
    to: PropertyKind,
) -> Option<PropertyValueEnum<M>> {
    match to {
        to if to == value.kind() => Some(value),

        PropertyKind::Optional => Some(values::Optional::try_from(value).ok()?.into()),

        PropertyKind::Hash => match value {
            PropertyValueEnum::String(str) => {
                Some(values::Hash::new_with_meta(BinHash::hash_str(&str), str.meta).into())
            }
            _ => None,
        },
        PropertyKind::ObjectLink => match value {
            PropertyValueEnum::Hash(hash) => {
                Some(values::ObjectLink::new_with_meta(*hash, hash.meta).into())
            }
            PropertyValueEnum::String(str) => {
                Some(values::ObjectLink::new_with_meta(BinHash::hash_str(&str), str.meta).into())
            }
            _ => None,
        },
        PropertyKind::WadChunkLink => match value {
            PropertyValueEnum::Hash(hash) => Some(
                values::WadChunkLink::new_with_meta(WadHash((**hash).into()), hash.meta).into(),
            ),
            PropertyValueEnum::String(str) => Some(
                values::WadChunkLink::new_with_meta(WadHash::hash_str(str.as_str()), str.meta)
                    .into(),
            ),
            _ => None,
        },
        PropertyKind::BitBool => match value {
            PropertyValueEnum::Bool(bool) => {
                Some(values::BitBool::new_with_meta(*bool, bool.meta).into())
            }
            _ => None,
        },
        PropertyKind::Bool => match value {
            PropertyValueEnum::BitBool(bool) => {
                Some(values::Bool::new_with_meta(*bool, bool.meta).into())
            }
            _ => None,
        },
        _ => None,
    }
}

fn resolve_rito_type(
    ctx: &mut Ctx<'_>,
    visit_ctx: &VisitCtx,
    tree: &Node,
) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.get(visit_ctx.cst).iter();

    let base = c.expect_token(visit_ctx.cst, TokenKind::Name)?;
    let base_span = base.span;

    let base = PropertyKind::from_rito_name(&ctx.text[base.span]).ok_or(UnknownType(base.span))?;

    let subtypes = match c.clone().find_map(|c| {
        c.tree(visit_ctx.cst)
            .filter(|t| t.kind == Kind::TypeArgList)
    }) {
        Some(subtypes) => {
            let subtypes_span = subtypes.span;

            let expected = base.subtype_count();

            if expected == 0 {
                return Err(UnexpectedSubtypes {
                    span: subtypes_span,
                    base_type: base_span,
                });
            }

            let subtypes = subtypes
                .children
                .get(visit_ctx.cst)
                .iter()
                .filter_map(|c| c.tree(visit_ctx.cst).filter(|t| t.kind == Kind::TypeArg))
                .map(|t| {
                    let resolved = PropertyKind::from_rito_name(&ctx.text[t.span]);
                    if resolved.is_none() {
                        ctx.diagnostics.push(UnknownType(t.span).unwrap());
                    }
                    (resolved, t.span)
                })
                .collect::<Vec<_>>();

            if subtypes.len() > expected.into() {
                return Err(SubtypeCountMismatch {
                    span: subtypes[expected as _..]
                        .iter()
                        .map(|s| s.1)
                        .reduce(|acc, s| Span::new(acc.start, s.end))
                        .unwrap_or(subtypes_span),
                    got: subtypes.len() as u8,
                    expected,
                });
            }
            if subtypes.len() < expected.into() {
                return Err(SubtypeCountMismatch {
                    span: subtypes.last().map(|s| s.1).unwrap_or(subtypes_span),
                    got: subtypes.len() as u8,
                    expected,
                });
            }

            let mut subtypes = subtypes.iter();
            [
                subtypes.next().and_then(|s| s.0),
                subtypes.next().and_then(|s| s.0),
            ]
        }
        None => [None, None],
    };

    Ok(RitoType { base, subtypes })
}

fn resolve_hash(ctx: &Ctx, span: Span) -> Result<PropertyValueEnum<Span>, Diagnostic> {
    // TODO: better errs here?
    let src = ctx.text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;

    // since we can't know whether bin/wad was intended, we will just try fit it in the smallest hash that allows it.
    // we can then safely coerce the type upwards when we are given type information
    Ok(match BinHash::from_str_radix(src, 16) {
        Ok(hash) => PropertyValueEnum::Hash(values::Hash::new_with_meta(hash, span)),
        Err(_) => match WadHash::from_str_radix(src, 16) {
            Ok(hash) => {
                PropertyValueEnum::WadChunkLink(values::WadChunkLink::new_with_meta(hash, span))
            }
            Err(_) => return Err(InvalidHash(span)),
        },
    })
}

fn resolve_literal(
    ctx: &mut Ctx,
    token: &Token,
    kind_hint: Option<RitoType>,
) -> Result<Option<PropertyValueEnum<Span>>, Diagnostic> {
    use PropertyKind as K;
    use PropertyValueEnum as P;
    Ok(Some(match token {
        Token {
            kind: TokenKind::String,
            span,
        } => values::String::new_with_meta(
            ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
            *span,
        )
        .into(),

        Token {
            kind: TokenKind::True,
            span,
        } => values::Bool::new_with_meta(true, *span).into(),
        Token {
            kind: TokenKind::False,
            span,
        } => values::Bool::new_with_meta(false, *span).into(),

        Token {
            kind: TokenKind::HexLit,
            span,
        } => resolve_hash(ctx, *span)?,
        Token {
            kind: TokenKind::Number,
            span,
        } => {
            let txt = &ctx.text[span];
            let Some(kind_hint) = kind_hint else {
                return Err(AmbiguousNumeric(*span));
            };

            let txt = match txt.contains('_') {
                true => Cow::Owned(txt.replace('_', "")),
                false => Cow::Borrowed(txt),
            };

            let kind_hint = match kind_hint.base {
                K::Optional => kind_hint.value_subtype().unwrap(),
                base => base,
            };

            match kind_hint {
                K::U8 => P::U8(values::U8::new_with_meta(
                    txt.parse::<u8>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U16 => P::U16(values::U16::new_with_meta(
                    txt.parse::<u16>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U32 => P::U32(values::U32::new_with_meta(
                    txt.parse::<u32>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::U64 => P::U64(values::U64::new_with_meta(
                    txt.parse::<u64>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I8 => P::I8(values::I8::new_with_meta(
                    txt.parse::<i8>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I16 => P::I16(values::I16::new_with_meta(
                    txt.parse::<i16>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I32 => P::I32(values::I32::new_with_meta(
                    txt.parse::<i32>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::I64 => P::I64(values::I64::new_with_meta(
                    txt.parse::<i64>()
                        .map_err(|e| Diagnostic::ParseNumericError {
                            expected: kind_hint,
                            error: Some(*e.kind()),
                            span: *span,
                        })?,
                    *span,
                )),
                K::F32 => P::F32(values::F32::new_with_meta(
                    txt.parse().map_err(|_| Diagnostic::ParseNumericError {
                        expected: kind_hint,
                        error: None,
                        span: *span,
                    })?,
                    *span,
                )),
                _ => {
                    return Err(TypeMismatch {
                        span: *span,
                        expected: RitoType::simple(kind_hint),
                        expected_span: None, // TODO: would be nice here
                        got: RitoTypeOrVirtual::numeric(),
                    });
                }
            }
        }
        _ => return Ok(None),
    }))
}

pub(crate) fn resolve_value(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    kind_hint: Option<RitoType>,
) -> Result<Option<PropertyValueEnum<Span>>, Diagnostic> {
    use PropertyKind as K;
    use PropertyValueEnum as P;

    let Some(child) = tree.children.get(visit_ctx.cst).first() else {
        return Ok(None);
    };
    Ok(Some(match child.tree(visit_ctx.cst) {
        Some(Node {
            kind: Kind::Class,
            children,
            span,
            ..
        }) => {
            let Some(kind_hint) = kind_hint else {
                return Ok(None); // TODO: err
            };
            let Some(class) = children
                .get(visit_ctx.cst)
                .first()
                .and_then(|t| t.token(visit_ctx.cst))
            else {
                return Err(InvalidHash(*span));
            };

            let class_hash = match class {
                Token {
                    kind: TokenKind::Name,
                    span,
                } => BinHash::hash_str(&ctx.text[span]),
                Token {
                    kind: TokenKind::HexLit,
                    span,
                } => match resolve_hash(ctx, *span)? {
                    PropertyValueEnum::Hash(hash) => *hash,
                    value => {
                        return Err(TypeMismatch {
                            span: *value.meta(),
                            expected: RitoType::simple(PropertyKind::Hash),
                            expected_span: None,
                            got: value.rito_type().into(),
                        });
                    }
                },
                _ => {
                    return Err(InvalidHash(class.span));
                }
            };
            match kind_hint.base {
                K::Struct => P::Struct(values::Struct {
                    class_hash,
                    meta: class.span,
                    properties: Default::default(),
                }),
                K::Embedded => P::Embedded(values::Embedded(values::Struct {
                    class_hash,
                    meta: class.span,
                    properties: Default::default(),
                })),
                other => {
                    trace!("can't create class value from kind {other:?}");
                    return Err(TypeMismatch {
                        span: class.span,
                        expected: RitoType::simple(other),
                        expected_span: None,
                        got: RitoTypeOrVirtual::StructOrEmbedded,
                    });
                }
            }
        }
        Some(Node {
            kind: Kind::Literal,
            children,
            ..
        }) => {
            let Some(child) = children.get(visit_ctx.cst).first() else {
                return Ok(None);
            };
            return resolve_literal(ctx, child.token(visit_ctx.cst).unwrap(), kind_hint);
        }
        _ => return Ok(None),
    }))
}

pub(crate) fn resolve_entry(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    parent_value_kind: Option<RitoType>,
) -> Result<IrEntry, MaybeSpanDiag> {
    let mut c = tree.children.get(visit_ctx.cst).iter();

    let key = c.expect_tree(visit_ctx.cst, Kind::EntryKey)?;

    let key = match key
        .children
        .get(visit_ctx.cst)
        .first()
        .ok_or(InvalidHash(key.span))?
        .token(visit_ctx.cst)
    {
        Some(Token {
            kind: TokenKind::Name,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(ctx.text[span].into(), *span)),
        Some(Token {
            kind: TokenKind::String,
            span,
        }) => PropertyValueEnum::from(values::String::new_with_meta(
            ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
            *span,
        )),
        Some(Token {
            kind: TokenKind::HexLit,
            span,
        }) => resolve_hash(ctx, *span)?,
        Some(token) => resolve_literal(
            ctx,
            token,
            parent_value_kind
                .and_then(|k| k.subtypes[0])
                .map(RitoType::simple),
        )?
        .ok_or(CustomSpan("erm idk bad literal", key.span))?,
        _ => {
            return Err(InvalidHash(key.span).into());
        }
    };

    let parent_value_kind = parent_value_kind
        .and_then(|p| p.value_subtype())
        .map(RitoType::simple);

    let kind = c
        .clone()
        .find_map(|c| c.tree(visit_ctx.cst).filter(|t| t.kind == Kind::TypeExpr));
    let kind_span = kind.map(|k| k.span);
    let kind = kind
        .map(|t| resolve_rito_type(ctx, visit_ctx, t))
        .transpose()?;

    let value = c.expect_tree(visit_ctx.cst, Kind::EntryValue)?;
    let value_span = value.span;

    // entries: map[string, u8] = {
    //     "bad": string = "string"
    //              ^
    // }
    if let Some(parent) = parent_value_kind.as_ref() {
        if let Some((kind, kind_span)) = kind.as_ref().zip(kind_span) {
            if parent != kind {
                ctx.diagnostics.push(
                    TypeMismatch {
                        span: kind_span,
                        expected: *parent,
                        expected_span: None, // TODO: would be nice here
                        got: (*kind).into(),
                    }
                    .unwrap(),
                );
                return Ok(IrEntry {
                    key,
                    value: parent.make_default(value.span),
                });
            }
        }
    }

    let kind = kind.or(parent_value_kind);

    let resolved_val = match resolve_value(ctx, visit_ctx, value, kind) {
        Ok(v) => v,
        Err(e) => Some(match kind {
            Some(kind) => {
                ctx.diagnostics.push(e.default_span(tree.span));
                kind.make_default(value.span)
            }
            None => {
                return Err(e.into());
            }
        }),
    };

    let resolved_val = resolved_val.map(|value| match kind {
        Some(kind) if value.kind() == kind.base => value,
        Some(kind) => coerce_type(value.clone(), kind.base).unwrap_or(value),
        None => value,
    });

    let value = match (kind, resolved_val) {
        (None, Some(value)) => value,
        (None, None) => return Err(MissingType(*key.meta()).into()),
        (Some(kind), Some(ivalue)) => match ivalue.kind() == kind.base {
            true => ivalue,
            false => {
                return Err(TypeMismatch {
                    span: *ivalue.meta(),
                    expected: kind,
                    expected_span: kind_span,
                    got: ivalue.rito_type().into(),
                }
                .into())
            }
        },
        (Some(kind), _) => kind.make_default(value_span),
    };

    Ok(IrEntry { key, value })
}
