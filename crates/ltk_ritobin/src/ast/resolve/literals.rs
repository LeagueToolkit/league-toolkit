use std::{borrow::Cow, str::FromStr};

use ltk_hash::{BinHash, WadHash};
use ltk_meta::{property::values, PropertyKind};

use crate::{
    ast::{
        diagnostics::{Diagnostic, RitoTypeOrVirtual},
        hash::{HashedLiteral, Originally},
        Value,
    },
    parse::{Span, Token, TokenKind},
    RitoType, Spanned,
};

use Diagnostic::*;

impl Value {
    pub(crate) fn eval_unknown_hash(text: &str, span: Span) -> Result<Self, Diagnostic> {
        // TODO: better errs here?
        let src = text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;

        // since we can't know whether bin/wad was intended, we will just try fit it in the smallest hash that allows it.
        // we can then safely coerce the type upwards when we are given type information
        Ok(match BinHash::from_str_radix(src, 16) {
            Ok(hash) => Self::Hash(HashedLiteral::new(span, Originally::HexLit, hash)),
            Err(_) => match WadHash::from_str_radix(src, 16) {
                Ok(hash) => Self::WadChunkLink(HashedLiteral::new(span, Originally::HexLit, hash)),
                Err(_) => return Err(InvalidHash(span)),
            },
        })
    }
}

pub(crate) fn eval_hash<H: ltk_hash::Hash + FromStr>(
    text: &str,
    span: Span,
) -> Result<HashedLiteral<H>, Diagnostic> {
    // TODO: better errs here?
    let src = text[span].strip_prefix("0x").ok_or(InvalidHash(span))?;
    H::from_str(src)
        .map_err(|_| InvalidHash(span))
        .map(|value| HashedLiteral::new(span, Originally::HexLit, value))
}

fn parse_int<T: std::str::FromStr<Err = std::num::ParseIntError>>(
    txt: &str,
    kind_hint: PropertyKind,
    span: Span,
    wrap: impl FnOnce(T, Span) -> Value,
) -> Result<Value, Diagnostic> {
    txt.parse::<T>()
        .map(|v| wrap(v, span))
        .map_err(|e| Diagnostic::ParseNumericError {
            expected: kind_hint,
            error: Some(*e.kind()),
            span,
        })
}

impl Value {
    pub(crate) fn eval(
        text: &str,
        token: &Token,
        kind_hint: Option<RitoType>,
        kind_hint_span: Option<Span>,
    ) -> Result<Option<Self>, Diagnostic> {
        use PropertyKind as K;
        Ok(Some(match token {
            Token {
                kind: TokenKind::String,
                span,
            } => Self::String(Spanned::new(
                *span,
                text[Span::new(span.start + 1, span.end - 1)].into(),
            )),

            Token {
                kind: TokenKind::True,
                span,
            } => Self::bool(*span, true),
            Token {
                kind: TokenKind::False,
                span,
            } => Self::bool(*span, false),

            Token {
                kind: TokenKind::HexLit,
                span,
            } => Self::eval_unknown_hash(text, *span)?,
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
                    K::Optional => kind_hint.value_subtype().unwrap_or(kind_hint.base),
                    base => base,
                };

                match kind_hint {
                    K::U8 => parse_int::<u8>(&txt, kind_hint, *span, |v, s| {
                        Self::U8(values::U8::new_with_meta(v, s))
                    })?,
                    K::U16 => parse_int::<u16>(&txt, kind_hint, *span, |v, s| {
                        Self::U16(values::U16::new_with_meta(v, s))
                    })?,
                    K::U32 => parse_int::<u32>(&txt, kind_hint, *span, |v, s| {
                        Self::U32(values::U32::new_with_meta(v, s))
                    })?,
                    K::U64 => parse_int::<u64>(&txt, kind_hint, *span, |v, s| {
                        Self::U64(values::U64::new_with_meta(v, s))
                    })?,
                    K::I8 => parse_int::<i8>(&txt, kind_hint, *span, |v, s| {
                        Self::I8(values::I8::new_with_meta(v, s))
                    })?,
                    K::I16 => parse_int::<i16>(&txt, kind_hint, *span, |v, s| {
                        Self::I16(values::I16::new_with_meta(v, s))
                    })?,
                    K::I32 => parse_int::<i32>(&txt, kind_hint, *span, |v, s| {
                        Self::I32(values::I32::new_with_meta(v, s))
                    })?,
                    K::I64 => parse_int::<i64>(&txt, kind_hint, *span, |v, s| {
                        Self::I64(values::I64::new_with_meta(v, s))
                    })?,
                    K::F32 => Self::F32(values::F32::new_with_meta(
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
) -> Result<Option<Value>, Diagnostic> {
    Value::eval(text, token, kind_hint, kind_hint_span)
}
