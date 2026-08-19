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

pub trait CanCoerce {
    fn can_coerce(self, from: Self) -> bool;
}

pub trait CoerceFrom {
    fn coerce_from<M: Debug + Default>(
        self,
        value: PropertyValueEnum<M>,
    ) -> Option<PropertyValueEnum<M>>;
}

impl CanCoerce for PropertyKind {
    fn can_coerce(self, from: Self) -> bool {
        let to = self;
        if to == from {
            return true;
        }
        use PropertyKind as K;
        match (to, from) {
            (K::Optional, from) if !from.is_container() => true,
            (K::Hash, K::String)
            | (K::WadChunkLink | K::ObjectLink, K::Hash | K::String)
            | (K::BitBool | K::Bool, K::Bool | K::BitBool) => true,
            _ => false,
        }
    }
}
impl CanCoerce for RitoType {
    fn can_coerce(self, from: Self) -> bool {
        if !self.base.can_coerce(from.base) {
            return false;
        }
        for i in 0..1 {
            if (self.subtypes[i].zip(from.subtypes[i]))
                .is_some_and(|(to, from)| !to.can_coerce(from))
            {
                return false;
            }
        }
        true
    }
}
impl CoerceFrom for PropertyKind {
    fn coerce_from<M: Debug + Default>(
        self,
        value: PropertyValueEnum<M>,
    ) -> Option<PropertyValueEnum<M>> {
        let to = self;
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
                PropertyValueEnum::String(str) => Some(
                    values::ObjectLink::new_with_meta(BinHash::hash_str(&str), str.meta).into(),
                ),
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
}

pub fn resolve_rito_type(
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

            if subtypes.len() != expected.into() {
                let span = if subtypes.len() > expected.into() {
                    subtypes[expected as _..]
                        .iter()
                        .map(|s| s.1)
                        .reduce(|acc, s| Span::new(acc.start, s.end))
                        .unwrap_or(subtypes_span)
                } else {
                    subtypes.last().map(|s| s.1).unwrap_or(subtypes_span)
                };
                return Err(SubtypeCountMismatch {
                    span,
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

fn parse_int<T: std::str::FromStr<Err = std::num::ParseIntError>>(
    txt: &str,
    kind_hint: PropertyKind,
    span: Span,
    wrap: impl FnOnce(T, Span) -> PropertyValueEnum<Span>,
) -> Result<PropertyValueEnum<Span>, Diagnostic> {
    txt.parse::<T>()
        .map(|v| wrap(v, span))
        .map_err(|e| Diagnostic::ParseNumericError {
            expected: kind_hint,
            error: Some(*e.kind()),
            span,
        })
}

/// Resolves a single literal token into the value it spells.
///
/// - `ctx` - typecheck state; diagnostics found along the way are pushed here
/// - `token` - the literal to resolve
/// - `kind_hint` - the type to read the literal as. A number, `true` or a string can be several
///   types, so without a hint an ambiguous literal cannot be resolved at all
/// - `kind_hint_span` - where `kind_hint` was written, so a mismatch can point at it
///
/// # Errors
/// If the literal does not fit `kind_hint`, or if it is ambiguous and there is no hint to pick
/// with - a bare `5` on its own has no type.
fn resolve_literal(
    ctx: &mut Ctx,
    token: &Token,
    kind_hint: Option<RitoType>,
    kind_hint_span: Option<Span>,
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
                K::U8 => parse_int::<u8>(&txt, kind_hint, *span, |v, s| {
                    P::U8(values::U8::new_with_meta(v, s))
                })?,
                K::U16 => parse_int::<u16>(&txt, kind_hint, *span, |v, s| {
                    P::U16(values::U16::new_with_meta(v, s))
                })?,
                K::U32 => parse_int::<u32>(&txt, kind_hint, *span, |v, s| {
                    P::U32(values::U32::new_with_meta(v, s))
                })?,
                K::U64 => parse_int::<u64>(&txt, kind_hint, *span, |v, s| {
                    P::U64(values::U64::new_with_meta(v, s))
                })?,
                K::I8 => parse_int::<i8>(&txt, kind_hint, *span, |v, s| {
                    P::I8(values::I8::new_with_meta(v, s))
                })?,
                K::I16 => parse_int::<i16>(&txt, kind_hint, *span, |v, s| {
                    P::I16(values::I16::new_with_meta(v, s))
                })?,
                K::I32 => parse_int::<i32>(&txt, kind_hint, *span, |v, s| {
                    P::I32(values::I32::new_with_meta(v, s))
                })?,
                K::I64 => parse_int::<i64>(&txt, kind_hint, *span, |v, s| {
                    P::I64(values::I64::new_with_meta(v, s))
                })?,
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
                        expected_span: kind_hint_span,
                        got: RitoTypeOrVirtual::numeric(),
                    });
                }
            }
        }
        _ => return Ok(None),
    }))
}

/// Resolves an `EntryValue` or `ListItem` tree into the value it describes.
///
/// - `ctx` - typecheck state; diagnostics found along the way are pushed here
/// - `visit_ctx` - the CST being walked
/// - `tree` - the tree holding the value
/// - `kind_hint` - the type the value is expected to have, used to resolve literals that cannot
///   type themselves - a bare `5` is only a `u8` because something said so
/// - `kind_hint_span` - where `kind_hint` was written, so a mismatch can point at it
///
/// # Returns
/// `Ok(None)` when the tree holds nothing resolvable, which the caller reports in its own terms -
/// an empty list item is a different mistake from an empty entry.
///
/// # Errors
/// If the value cannot be read as `kind_hint` - a literal of the wrong type, a number that does
/// not fit, or an ambiguous literal with no hint to resolve it against.
pub(crate) fn resolve_value(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    kind_hint: Option<RitoType>,
    kind_hint_span: Option<Span>,
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

        // Matches a block with no class name - { .. }
        Some(
            block @ Node {
                kind: Kind::Block, ..
            },
        ) => {
            let Some(kind_hint) = kind_hint else {
                return Ok(None);
            };
            if !matches!(kind_hint.base, K::Struct | K::Embedded) {
                return Ok(None);
            }

            // Structs and embedded values must have a class name before the block
            return Err(MissingClassName {
                span: block.open_brace_span(visit_ctx.cst),
                expected: kind_hint,
            });
        }
        Some(Node {
            kind: Kind::Literal,
            children,
            ..
        }) => {
            let Some(child) = children.get(visit_ctx.cst).first() else {
                return Ok(None);
            };
            return resolve_literal(
                ctx,
                child.token(visit_ctx.cst).unwrap(),
                kind_hint,
                kind_hint_span,
            );
        }
        _ => return Ok(None),
    }))
}

/// Resolves an `Entry` tree into the key/value pair it describes.
///
/// - `ctx` - typecheck state; diagnostics found along the way are pushed here
/// - `visit_ctx` - the CST being walked
/// - `tree` - the `Entry` tree to resolve
/// - `parent_value_kind` - the type the enclosing container gives its values, so an entry that
///   wrote no `: type` can still be resolved. `None` at the root
/// - `parent_type_span` - where that type was written, so a mismatch can point at it. `None` at
///   the root, and for a container that took its type from a subtype rather than a type expression
///
/// # Errors
/// If the tree is not a well-formed entry - no key, no value, or a key that is not a hash.
/// A well-formed entry that fails to type-check resolves to a default value instead, so the walk
/// keeps going and the diagnostic lands in `ctx`.
pub(crate) fn resolve_entry(
    ctx: &mut Ctx,
    visit_ctx: &VisitCtx,
    tree: &Node,
    parent_value_kind: Option<RitoType>,
    parent_type_span: Option<Span>,
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
        }) => {
            // We can support quoted property names by just hashing the string
            // The original ritobin compiler has no support for it and we prefer
            // unquoted names - emit diagnostic
            if let Some(parent) = parent_value_kind
                .filter(|p| matches!(p.base, PropertyKind::Struct | PropertyKind::Embedded))
            {
                ctx.diagnostics.push(
                    QuotedPropertyName {
                        span: *span,
                        parent,
                    }
                    .unwrap(),
                );
            }

            PropertyValueEnum::from(values::String::new_with_meta(
                ctx.text[Span::new(span.start + 1, span.end - 1)].into(),
                *span,
            ))
        }
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
            parent_type_span,
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
            if !parent.can_coerce(*kind) {
                ctx.diagnostics.push(
                    TypeMismatch {
                        span: kind_span,
                        expected: *parent,
                        expected_span: parent_type_span,
                        got: (*kind).into(),
                    }
                    .unwrap(),
                );
                return Ok(IrEntry {
                    key,
                    // we fell back to the parent's type, so that is what declared this value
                    type_span: parent_type_span,
                    value: parent.make_default(value.span),
                });
            }
        }
    }

    let kind = kind.or(parent_value_kind);
    let type_span = kind_span.or(parent_type_span);

    let resolved_val = match resolve_value(ctx, visit_ctx, value, kind, type_span) {
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
        Some(kind) => kind.base.coerce_from(value.clone()).unwrap_or(value),
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

    Ok(IrEntry {
        key,
        value,
        type_span,
    })
}
