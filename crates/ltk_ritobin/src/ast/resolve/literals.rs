use std::{borrow::Cow, fmt::Display, num::IntErrorKind, str::FromStr};

use ltk_hash::{BinHash, WadHash};
use ltk_meta::PropertyKind;

use crate::{
    ast::{
        diagnostics::{RitoTypeOrVirtual, TypeMismatch},
        hash::{HashedLiteral, Originally},
        Value,
    },
    escaping,
    parse::{Token, TokenKind},
    RitoType, Spanned, SpannedExt,
};
use span::Span;

use ValueEvalError as E;

impl Value {
    pub(crate) fn eval_unknown_hash(text: &str, span: Span) -> Result<Self, ValueEvalError> {
        // TODO: better errs here?
        let src = text[span].strip_prefix("0x").ok_or(E::InvalidHash(span))?;

        // since we can't know whether bin/wad was intended, we will just try fit it in the smallest hash that allows it.
        // we can then safely coerce the type upwards when we are given type information
        Ok(match BinHash::from_str_radix(src, 16) {
            Ok(hash) => Self::Hash(HashedLiteral::new(span, Originally::HexLit, hash)),
            Err(_) => match WadHash::from_str_radix(src, 16) {
                Ok(hash) => Self::WadChunkLink(HashedLiteral::new(span, Originally::HexLit, hash)),
                Err(_) => return Err(E::InvalidHash(span)),
            },
        })
    }
}

pub(crate) fn eval_hash<H: ltk_hash::Hash + FromStr>(
    text: &str,
    span: Span,
) -> Result<HashedLiteral<H>, ValueEvalError> {
    // TODO: better errs here?
    let src = text[span].strip_prefix("0x").ok_or(E::InvalidHash(span))?;
    H::from_str(src)
        .map_err(|_| E::InvalidHash(span))
        .map(|value| HashedLiteral::new(span, Originally::HexLit, value))
}

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub struct ParseNumericError {
    pub expected: PropertyKind,
    pub error: Option<std::num::IntErrorKind>,
    pub span: Span,
}

impl Display for ParseNumericError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let reason = match self.error {
            Some(IntErrorKind::Empty) => "cannot parse integer from empty literal",
            Some(IntErrorKind::InvalidDigit) => "invalid digit found in literal",
            Some(IntErrorKind::PosOverflow) => "number too large to fit in target type",
            Some(IntErrorKind::NegOverflow) => "number too small to fit in target type",
            Some(IntErrorKind::Zero) => "number would be zero",
            _ => "invalid literal",
        };
        write!(
            f,
            "Could not parse {} - {reason}",
            RitoType::simple(self.expected)
        )
    }
}

fn parse_int<T: std::str::FromStr<Err = std::num::ParseIntError>>(
    txt: &str,
    kind_hint: PropertyKind,
    span: Span,
    wrap: impl FnOnce(T, Span) -> Value,
) -> Result<Value, ParseNumericError> {
    txt.parse::<T>()
        .map(|v| wrap(v, span))
        .map_err(|e| ParseNumericError {
            expected: kind_hint,
            error: Some(*e.kind()),
            span,
        })
}

fn parse_f32(txt: &str, span: Span) -> Result<Value, ParseNumericError> {
    Ok(Value::F32(
        txt.parse::<f32>()
            .map_err(|_| ParseNumericError {
                expected: PropertyKind::F32,
                error: None,
                span,
            })?
            .with_span(span),
    ))
}

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum ValueEvalError {
    #[error("Ambiguous numeric literal - it needs a type to be resolved against")]
    AmbiguousNumeric(Span),
    #[error("Invalid hash")]
    InvalidHash(Span),
    #[error("Unexpected token - {:?}", .0.kind)]
    UnexpectedToken(Token),

    #[error(transparent)]
    ParseNumericError(#[from] ParseNumericError),
    #[error(transparent)]
    TypeMismatch(#[from] TypeMismatch),
    #[error(transparent)]
    InvalidEscape(#[from] Spanned<escaping::InvalidEscape>),
}

impl ValueEvalError {
    pub fn span(&self) -> Span {
        match self {
            ValueEvalError::AmbiguousNumeric(span) | ValueEvalError::InvalidHash(span) => *span,
            ValueEvalError::UnexpectedToken(token) => token.span,
            ValueEvalError::ParseNumericError(err) => err.span,
            ValueEvalError::TypeMismatch(err) => err.span,
            ValueEvalError::InvalidEscape(err) => err.span,
        }
    }
}

impl Value {
    /// Evaluate a literal token into a value
    ///
    /// # Errors
    /// If the literal does not fit `kind_hint`, or if it is ambiguous and there is no hint to pick
    /// with - a bare `5` on its own has no type.
    pub(crate) fn eval(
        text: &str,
        token: &Token,
        numeric_type: Option<RitoType>,
        numeric_type_span: Option<Span>,
    ) -> Result<Self, ValueEvalError> {
        use PropertyKind as K;
        Ok(match token {
            Token {
                kind: TokenKind::String,
                span,
            } => Self::String(Spanned::new(
                *span,
                escaping::unescape(&text[Span::new(span.start + 1, span.end - 1)])
                    .map_err(|e| e.with_span(*span))?,
            )),

            Token {
                kind: TokenKind::Null,
                span,
            } => Self::None(*span),

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
                // let Some(kind_hint) = numeric_hint else {
                //     return Err(E::AmbiguousNumeric(*span));
                // };

                let txt = match txt.contains('_') {
                    true => Cow::Owned(txt.replace('_', "")),
                    false => Cow::Borrowed(txt),
                };

                let numeric_hint = numeric_type.map(|h| match h.base {
                    K::Optional => h.value_subtype().unwrap_or(h.base),
                    base => base,
                });

                match numeric_hint {
                    Some(h @ K::U8) => {
                        parse_int::<u8>(&txt, h, *span, |v, s| Self::U8(Spanned::new(s, v)))?
                    }
                    Some(h @ K::U16) => {
                        parse_int::<u16>(&txt, h, *span, |v, s| Self::U16(Spanned::new(s, v)))?
                    }
                    Some(h @ K::U32) => {
                        parse_int::<u32>(&txt, h, *span, |v, s| Self::U32(Spanned::new(s, v)))?
                    }
                    Some(h @ K::U64) => {
                        parse_int::<u64>(&txt, h, *span, |v, s| Self::U64(Spanned::new(s, v)))?
                    }
                    Some(h @ K::I8) => {
                        parse_int::<i8>(&txt, h, *span, |v, s| Self::I8(Spanned::new(s, v)))?
                    }
                    Some(h @ K::I16) => {
                        parse_int::<i16>(&txt, h, *span, |v, s| Self::I16(Spanned::new(s, v)))?
                    }
                    Some(h @ K::I32) => {
                        parse_int::<i32>(&txt, h, *span, |v, s| Self::I32(Spanned::new(s, v)))?
                    }
                    Some(h @ K::I64) => {
                        parse_int::<i64>(&txt, h, *span, |v, s| Self::I64(Spanned::new(s, v)))?
                    }
                    Some(h @ K::F32) => Self::F32(Spanned::new(
                        *span,
                        txt.parse().map_err(|_| ParseNumericError {
                            expected: h,
                            error: None,
                            span: *span,
                        })?,
                    )),
                    Some(numeric_hint) => {
                        return Err(TypeMismatch {
                            span: *span,
                            expected: RitoType::simple(numeric_hint).into(),
                            expected_span: numeric_type_span,
                            got: RitoTypeOrVirtual::numeric(),
                        }
                        .into());
                    }
                    None => {
                        (parse_int::<u64>(&txt, K::U64, *span, |v, s| {
                            Self::U64(Spanned::new(s, v))
                        })
                        .or(parse_int::<i64>(&txt, K::I64, *span, |v, s| {
                            Self::I64(Spanned::new(s, v))
                        }))
                        .or(parse_f32(&txt, *span)))?
                    }
                }
            }
            token => return Err(E::UnexpectedToken(*token)),
        })
    }
}
