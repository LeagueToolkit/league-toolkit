//! Annotations: `@name(args)` lines that apply to the entry or list item after them.
//!
//! The crate gives an annotation no meaning. It parses the line, attaches it to its target and
//! keeps it in [`Annotations`], a side table on [`Ast`](crate::ast::Ast) keyed by target span. A
//! consumer reads the raw table through [`AnnotationRef`], or resolves it into its own type with
//! [`FromAnnotation`].
//!
//! The spec is `docs/design/ritobin-annotations.md`.

mod resolve;
mod source;
mod table;

pub use resolve::{FromAnnotation, Resolved};
pub use source::{AnnotationRef, ArgError, ArgErrorKind, FromArgument};
pub use table::{Annotations, SourceAnnotations};

use crate::{
    ast::{
        node::{root::Root, MapEntry},
        Property, RootEntry, Value,
    },
    span::Span,
};

/// One `@name(args)` as written in the source.
///
/// Names and flags are spans into the source the tree was built from; pair the annotation with
/// that source through [`with_source`](Self::with_source) to read them.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
pub struct Annotation {
    span: Span,
    name: Span,
    arguments: Vec<Argument>,
}

impl Annotation {
    pub(crate) fn new(span: Span, name: Span, arguments: Vec<Argument>) -> Self {
        Self {
            span,
            name,
            arguments,
        }
    }

    /// From `@` to the closing `)`, or to the end of the name when there are no arguments.
    #[must_use]
    pub fn span(&self) -> Span {
        self.span
    }

    /// The name only, without `@`.
    #[must_use]
    pub fn name(&self) -> Span {
        self.name
    }

    #[must_use]
    pub fn arguments(&self) -> &[Argument] {
        &self.arguments
    }

    /// Pairs this annotation with the source it was parsed from.
    ///
    /// # Panics
    ///
    /// The returned view panics when read if `text` is not that source.
    #[must_use]
    pub fn with_source<'a>(&'a self, text: &'a str) -> AnnotationRef<'a> {
        AnnotationRef::new(self, text)
    }
}

/// One item inside an annotation's parentheses.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
#[non_exhaustive]
pub enum Argument {
    /// A bare `name`.
    Flag(Span),
    /// A `name = literal`.
    Named { name: Span, value: Value },
}

impl Argument {
    #[must_use]
    pub fn name(&self) -> Span {
        match self {
            Self::Flag(name) | Self::Named { name, .. } => *name,
        }
    }

    /// The whole argument: the name, or `name = value`.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::Flag(name) => *name,
            Self::Named { name, value } => name.cover(value.span()),
        }
    }

    #[must_use]
    pub fn value(&self) -> Option<&Value> {
        match self {
            Self::Flag(_) => None,
            Self::Named { value, .. } => Some(value),
        }
    }
}

/// An annotation and the span of the node it applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
pub struct Attached<A> {
    target: Span,
    annotation: A,
}

impl<A> Attached<A> {
    pub(crate) fn new(target: Span, annotation: A) -> Self {
        Self { target, annotation }
    }

    /// The span of the node this annotation applies to.
    #[must_use]
    pub fn target(&self) -> Span {
        self.target
    }

    #[must_use]
    pub fn annotation(&self) -> &A {
        &self.annotation
    }

    #[must_use]
    pub fn into_annotation(self) -> A {
        self.annotation
    }
}

mod sealed {
    pub trait Sealed {}
}

/// A node an annotation can target.
///
/// Implemented for [`Root`], [`RootEntry`], [`Property`], [`MapEntry`], [`Value`] (as a list
/// item), and [`Span`] as the identity for a caller that already holds a target span. Sealed.
pub trait Annotatable: sealed::Sealed {
    /// The span annotations on this node are keyed by.
    fn target(&self) -> Span;
}

macro_rules! annotatable {
    ($($ty:ty => |$this:ident| $target:expr),* $(,)?) => {$(
        impl sealed::Sealed for $ty {}
        impl Annotatable for $ty {
            fn target(&self) -> Span {
                let $this = self;
                $target
            }
        }
    )*};
}

annotatable! {
    Root => |r| r.span(),
    RootEntry => |e| e.span(),
    Property => |p| p.span(),
    MapEntry => |e| e.span(),
    Value => |v| v.span(),
    Span => |s| *s,
}
