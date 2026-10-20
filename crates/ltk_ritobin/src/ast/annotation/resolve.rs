use crate::Spanned;

use super::{Annotation, AnnotationRef, Annotations, Attached};

/// Turns a raw annotation into a consumer's own type.
///
/// Most consumers implement this once per tool. A resolver that carries runtime state uses
/// [`Annotations::resolve_with`] instead.
pub trait FromAnnotation: Sized {
    type Error: std::error::Error;

    /// `Ok(None)` means the annotation is not this type's and is left unclaimed, for another
    /// tool to read.
    ///
    /// # Errors
    ///
    /// The annotation is this type's, but malformed. An [`ArgError`](super::ArgError) raised
    /// with `?` keeps the argument's span; [`AnnotationRef::error`] spans anything else at the
    /// annotation.
    fn from_annotation(annotation: AnnotationRef<'_>)
        -> Result<Option<Self>, Spanned<Self::Error>>;
}

impl Annotations {
    /// Resolves every annotation into `T`. See [`FromAnnotation`].
    #[must_use]
    pub fn resolve<T: FromAnnotation>(&self, text: &str) -> Resolved<T, T::Error> {
        self.resolve_with(text, T::from_annotation)
    }

    /// Resolves every annotation through `resolver`, for a resolver that carries state.
    ///
    /// The contract is that of [`FromAnnotation::from_annotation`].
    #[must_use]
    pub fn resolve_with<T, E, F>(&self, text: &str, mut resolver: F) -> Resolved<T, E>
    where
        F: FnMut(AnnotationRef<'_>) -> Result<Option<T>, Spanned<E>>,
    {
        let mut resolved = Resolved {
            annotations: Annotations::default(),
            errors: Vec::new(),
            unclaimed: Vec::new(),
        };
        for attached in self {
            match resolver(attached.annotation().with_source(text)) {
                Ok(Some(value)) => resolved.annotations.attach(value, attached.target()),
                Ok(None) => resolved.unclaimed.push(attached.clone()),
                Err(error) => resolved.errors.push(error),
            }
        }
        resolved
    }
}

/// The result of [`Annotations::resolve`] or [`Annotations::resolve_with`].
///
/// A failed annotation is in [`errors`](Self::errors) only; its target gets no entry in
/// [`annotations`](Self::annotations). The raw table is untouched.
#[derive(Debug, Clone)]
pub struct Resolved<T, E> {
    annotations: Annotations<T>,
    errors: Vec<Spanned<E>>,
    unclaimed: Vec<Attached<Annotation>>,
}

impl<T, E> Resolved<T, E> {
    #[must_use]
    pub fn annotations(&self) -> &Annotations<T> {
        &self.annotations
    }

    /// Each failure, spanned at the argument or annotation that caused it.
    #[must_use]
    pub fn errors(&self) -> &[Spanned<E>] {
        &self.errors
    }

    /// Raw annotations the resolver returned `Ok(None)` for, in source order.
    pub fn unclaimed(&self) -> impl Iterator<Item = &Attached<Annotation>> + '_ {
        self.unclaimed.iter()
    }

    #[must_use]
    pub fn into_parts(self) -> (Annotations<T>, Vec<Spanned<E>>) {
        (self.annotations, self.errors)
    }
}
