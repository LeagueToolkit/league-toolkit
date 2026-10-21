use ltk_hash::{BinHash, Hash as _, WadHash};

use crate::{ast::Value, span::Span, Spanned};

use super::{Annotation, Argument, FromAnnotation};

/// An [`Annotation`] paired with the source text it was parsed from.
#[derive(Debug, Clone, Copy)]
pub struct AnnotationRef<'a> {
    annotation: &'a Annotation,
    text: &'a str,
}

impl<'a> AnnotationRef<'a> {
    pub(crate) fn new(annotation: &'a Annotation, text: &'a str) -> Self {
        Self { annotation, text }
    }

    #[must_use]
    pub fn annotation(&self) -> &'a Annotation {
        self.annotation
    }

    /// The name of the annotation (`foo` in `@foo(..)`).
    #[must_use]
    pub fn name(&self) -> &'a str {
        &self.text[self.annotation.name]
    }

    /// Source text of an argument's name.
    #[must_use]
    pub fn arg_name(&self, argument: &Argument) -> &'a str {
        &self.text[argument.name()]
    }

    /// The value of the `name = value` argument.
    ///
    /// `Ok(None)` when the argument is absent or `name = null`.
    ///
    /// # Errors
    ///
    /// `WrongType` when the argument is a flag or a value `T` does not accept, `OutOfRange` when
    /// an integer does not fit `T`. Both are spanned at the argument.
    pub fn arg<T: FromArgument<'a>>(&self, name: &str) -> Result<Option<T>, ArgError> {
        let Some(argument) = self.find(name) else {
            return Ok(None);
        };
        match argument {
            Argument::Flag(span) => Err(ArgError {
                span: *span,
                kind: ArgErrorKind::WrongType {
                    expected: T::EXPECTED,
                },
            }),
            Argument::Named {
                value: Value::None(_),
                ..
            } => Ok(None),
            Argument::Named { value, .. } => {
                T::from_value(value, self.text)
                    .map(Some)
                    .map_err(|kind| ArgError {
                        span: value.span(),
                        kind,
                    })
            }
        }
    }

    /// Like [`arg`](Self::arg), but an absent or `null` argument is an error.
    ///
    /// # Errors
    ///
    /// As [`arg`](Self::arg), plus `Missing`, spanned at the annotation.
    pub fn require<T: FromArgument<'a>>(&self, name: &str) -> Result<T, ArgError> {
        self.arg(name)?.ok_or(ArgError {
            span: self.annotation.span,
            kind: ArgErrorKind::Missing,
        })
    }

    /// `true` for a bare `name` or `name = true`; `false` when absent, `null` or `name = false`.
    ///
    /// # Errors
    ///
    /// `WrongType` for any other value, spanned at it.
    pub fn flag(&self, name: &str) -> Result<bool, ArgError> {
        match self.find(name) {
            None
            | Some(Argument::Named {
                value: Value::None(_),
                ..
            }) => Ok(false),
            Some(Argument::Flag(_)) => Ok(true),
            Some(Argument::Named { value, .. }) => {
                bool::from_value(value, self.text).map_err(|kind| ArgError {
                    span: value.span(),
                    kind,
                })
            }
        }
    }

    /// Checks that every argument is one of `allowed`.
    ///
    /// # Errors
    ///
    /// `Unexpected`, spanned at the first argument that is not.
    pub fn only(&self, allowed: &[&str]) -> Result<(), ArgError> {
        match self
            .annotation
            .arguments
            .iter()
            .find(|a| !allowed.contains(&self.arg_name(a)))
        {
            Some(unexpected) => Err(ArgError {
                span: unexpected.span(),
                kind: ArgErrorKind::Unexpected,
            }),
            None => Ok(()),
        }
    }

    /// Resolves this one annotation into `T`. See [`FromAnnotation`].
    ///
    /// # Errors
    ///
    /// Those of [`FromAnnotation::resolve`].
    pub fn resolve<T: FromAnnotation>(&self) -> Result<T, Spanned<T::Error>> {
        T::resolve(*self)
    }

    /// Spans a consumer error at this annotation.
    #[must_use]
    pub fn error<E>(&self, error: E) -> Spanned<E> {
        Spanned::new(self.annotation.span, error)
    }

    fn find(&self, name: &str) -> Option<&'a Argument> {
        self.annotation
            .arguments
            .iter()
            .find(|a| self.arg_name(a) == name)
    }
}

/// Reading an argument failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind}")]
pub struct ArgError {
    pub span: Span,
    pub kind: ArgErrorKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ArgErrorKind {
    #[error("missing argument")]
    Missing,
    #[error("expected {expected}")]
    WrongType { expected: &'static str },
    #[error("value out of range for {ty}")]
    OutOfRange { ty: &'static str },
    #[error("unexpected argument")]
    Unexpected,
}

impl<E: From<ArgError>> From<ArgError> for Spanned<E> {
    fn from(error: ArgError) -> Self {
        Spanned::new(error.span, E::from(error))
    }
}

mod sealed {
    use crate::ast::Value;

    use super::ArgErrorKind;

    pub trait FromValue<'a>: Sized {
        /// What `WrongType` names when the value is not accepted.
        const EXPECTED: &'static str;
        fn from_value(value: &'a Value, text: &'a str) -> Result<Self, ArgErrorKind>;
    }
}

pub(crate) use sealed::FromValue;

/// A type an argument value converts to. Sealed.
///
/// | `T` | Accepts |
/// | --- | --- |
/// | `u8` to `u64`, `i8` to `i64` | an integer literal in range |
/// | `f32` | a number literal |
/// | `bool` | `true` or `false` |
/// | `&str` | a string literal |
/// | [`BinHash`] | a 32-bit hex literal, or a string, hashed |
/// | [`WadHash`] | a hex literal, or a string, hashed |
///
/// A string hashes the way a string key does in a `hash` or `link` value: lowercased FNV-1a for
/// [`BinHash`], lowercased XXH64 for [`WadHash`].
pub trait FromArgument<'a>: FromValue<'a> {}

macro_rules! from_argument_int {
    ($($ty:ty),* $(,)?) => {$(
        impl<'a> FromValue<'a> for $ty {
            const EXPECTED: &'static str = "integer";

            fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
                let out_of_range = ArgErrorKind::OutOfRange { ty: stringify!($ty) };
                match value {
                    Value::U64(v) => Self::try_from(v.value).map_err(|_| out_of_range),
                    Value::I64(v) => Self::try_from(v.value).map_err(|_| out_of_range),
                    _ => Err(ArgErrorKind::WrongType { expected: Self::EXPECTED }),
                }
            }
        }
        impl<'a> FromArgument<'a> for $ty {}
    )*};
}

from_argument_int!(u8, u16, u32, u64, i8, i16, i32, i64);

impl<'a> FromValue<'a> for f32 {
    const EXPECTED: &'static str = "number";

    fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
        match value {
            Value::F32(v) => Ok(v.value),
            Value::U64(v) => Ok(v.value as f32),
            Value::I64(v) => Ok(v.value as f32),
            _ => Err(ArgErrorKind::WrongType {
                expected: Self::EXPECTED,
            }),
        }
    }
}
impl<'a> FromArgument<'a> for f32 {}

impl<'a> FromValue<'a> for bool {
    const EXPECTED: &'static str = "bool";

    fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
        match value {
            Value::Bool(v) | Value::BitBool(v) => Ok(v.value),
            _ => Err(ArgErrorKind::WrongType {
                expected: Self::EXPECTED,
            }),
        }
    }
}
impl<'a> FromArgument<'a> for bool {}

impl<'a> FromValue<'a> for &'a str {
    const EXPECTED: &'static str = "string";

    fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
        match value {
            Value::String(v) => Ok(v.value.as_str()),
            _ => Err(ArgErrorKind::WrongType {
                expected: Self::EXPECTED,
            }),
        }
    }
}
impl<'a> FromArgument<'a> for &'a str {}

impl<'a> FromValue<'a> for BinHash {
    const EXPECTED: &'static str = "hash";

    fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
        match value {
            Value::Hash(v) => Ok(v.value),
            Value::String(v) => Ok(BinHash::hash_str(&v.value)),
            _ => Err(ArgErrorKind::WrongType {
                expected: Self::EXPECTED,
            }),
        }
    }
}
impl<'a> FromArgument<'a> for BinHash {}

impl<'a> FromValue<'a> for WadHash {
    const EXPECTED: &'static str = "hash";

    fn from_value(value: &'a Value, _: &'a str) -> Result<Self, ArgErrorKind> {
        match value {
            Value::WadChunkLink(v) => Ok(v.value),
            Value::Hash(v) => Ok(WadHash(u64::from(v.value.0))),
            Value::String(v) => Ok(WadHash::hash_str(&v.value)),
            _ => Err(ArgErrorKind::WrongType {
                expected: Self::EXPECTED,
            }),
        }
    }
}
impl<'a> FromArgument<'a> for WadHash {}
