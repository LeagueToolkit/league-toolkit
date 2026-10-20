use crate::span::Span;

use super::{Annotatable, Annotation, AnnotationRef, Attached};

/// Annotations keyed by the node they apply to.
///
/// Sorted by target start offset, so targets follow source order and a nested target follows the
/// one enclosing it. Annotations on one target keep source order.
///
/// `A` is [`Annotation`] on the table an [`Ast`](crate::ast::Ast) carries, and a consumer's type
/// on a table that [`resolve`](Self::resolve) produced.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
pub struct Annotations<A = Annotation> {
    attached: Vec<Attached<A>>,
}

impl<A> Default for Annotations<A> {
    fn default() -> Self {
        Self {
            attached: Vec::new(),
        }
    }
}

impl<A> Annotations<A> {
    /// Attaches `annotation` to `target`.
    ///
    /// Callers attach in source order; the sort order documented on the type is the caller's
    /// invariant, checked in debug builds.
    pub(crate) fn attach(&mut self, annotation: A, target: Span) {
        debug_assert!(
            self.attached
                .last()
                .is_none_or(|last| last.target.start <= target.start),
            "annotations attached out of source order: {target:?}"
        );
        self.attached.push(Attached::new(target, annotation));
    }

    /// Annotations direct on `node`, in source order.
    pub fn on(&self, node: &impl Annotatable) -> impl Iterator<Item = &A> + '_ {
        self.attached_on(node.target()).map(Attached::annotation)
    }

    /// Annotations direct on `node` or inherited from an enclosing node: outermost target first,
    /// source order within one target.
    pub fn applying_to(&self, node: &impl Annotatable) -> impl Iterator<Item = &Attached<A>> + '_ {
        let span = node.target();
        self.attached
            .iter()
            .take_while(move |a| a.target.start <= span.start)
            .filter(move |a| span.end <= a.target.end)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Attached<A>> {
        self.attached.iter()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.attached.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.attached.is_empty()
    }

    fn attached_on(&self, target: Span) -> impl Iterator<Item = &Attached<A>> + '_ {
        let from = self
            .attached
            .partition_point(|a| a.target.start < target.start);
        self.attached[from..]
            .iter()
            .take_while(move |a| a.target.start == target.start)
    }
}

impl<'a, A> IntoIterator for &'a Annotations<A> {
    type Item = &'a Attached<A>;
    type IntoIter = std::slice::Iter<'a, Attached<A>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Annotations {
    /// Binds the source text once, so every query yields a readable [`AnnotationRef`].
    ///
    /// # Panics
    ///
    /// The view panics when read if `text` is not the source the tree was built from.
    #[must_use]
    pub fn with_source<'a>(&'a self, text: &'a str) -> SourceAnnotations<'a> {
        SourceAnnotations { table: self, text }
    }
}

/// [`Annotations`] with the source bound. Same queries; items are [`AnnotationRef`].
#[derive(Debug, Clone, Copy)]
pub struct SourceAnnotations<'a> {
    table: &'a Annotations,
    text: &'a str,
}

impl<'a> SourceAnnotations<'a> {
    /// Annotations direct on `node`, in source order.
    pub fn on(&self, node: &impl Annotatable) -> impl Iterator<Item = AnnotationRef<'a>> + 'a {
        let text = self.text;
        self.table
            .on(node)
            .map(move |annotation| annotation.with_source(text))
    }

    /// Annotations direct on `node` or inherited from an enclosing node: outermost target first,
    /// source order within one target.
    pub fn applying_to(
        &self,
        node: &impl Annotatable,
    ) -> impl Iterator<Item = Attached<AnnotationRef<'a>>> + 'a {
        let text = self.text;
        self.table
            .applying_to(node)
            .map(move |a| a.with_source(text))
    }

    pub fn iter(&self) -> impl Iterator<Item = Attached<AnnotationRef<'a>>> + 'a {
        let text = self.text;
        self.table.iter().map(move |a| a.with_source(text))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.table.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

impl Attached<Annotation> {
    fn with_source<'a>(&'a self, text: &'a str) -> Attached<AnnotationRef<'a>> {
        Attached::new(self.target, self.annotation.with_source(text))
    }
}
