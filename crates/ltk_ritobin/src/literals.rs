use std::{borrow::Cow, fmt::Debug, str::FromStr};

use ltk_hash::{BinHash, Hash as _, WadHash};
use ltk_meta::{property::values, PropertyKind, PropertyValueEnum};

use crate::{
    parse::{Span, Token, TokenKind},
    typecheck::diagnostics::{Diagnostic, RitoTypeOrVirtual},
    RitoType,
};

use Diagnostic::*;

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

pub(crate) fn eval_unknown_hash(
    text: &str,
    span: Span,
) -> Result<PropertyValueEnum<Span>, Diagnostic> {
    // TODO: better errs here?
    let src = text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;

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

// pub(crate) fn eval_hash<H: ltk_hash::Hash + FromStr>(
//     text: &str,
//     span: Span,
// ) -> Result<H, Diagnostic> {
//     // TODO: better errs here?
//     let src = text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;
//     H::from_str(src).map_err(|_| InvalidHash(span))
// }

pub(crate) fn parse_int<T: std::str::FromStr<Err = std::num::ParseIntError>>(
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

/// Evaluate a literal token into a value
///
/// # Errors
/// If the literal does not fit `kind_hint`, or if it is ambiguous and there is no hint to pick
/// with - a bare `5` on its own has no type.
pub(crate) fn eval(
    text: &str,
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
            text[Span::new(span.start + 1, span.end - 1)].into(),
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
        } => eval_unknown_hash(text, *span)?,
        Token {
            kind: TokenKind::Number,
            span,
        } => {
            let txt = &text[span];
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
