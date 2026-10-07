use span::{DebugSpans, Span};

use crate::{ast::Value, Spanned, SpannedExt};

#[derive(Debug, Clone, DebugSpans)]
pub struct Annotation {
    pub span: Span,
    pub name: Span,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, DebugSpans)]
pub enum Argument {
    Bare(Span),
    Named { name: Span, value: Value },
}

#[derive(Debug, Clone, DebugSpans)]
pub struct Annotations<A = Annotation> {
    nodes: Vec<Spanned<A>>,
}

impl<A> Default for Annotations<A> {
    fn default() -> Self {
        Self {
            nodes: Default::default(),
        }
    }
}

impl<A> Annotations<A> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, annotation: A, span: Span) {
        let set = annotation.with_span(span);
        self.nodes.push(set);
    }
    pub fn extend(&mut self, annotations: impl IntoIterator<Item = A>, target: Span) {
        for a in annotations.into_iter() {
            let a = a.with_span(target);
            self.nodes.push(a);
        }
    }

    pub fn map<T, F>(self, mut map: F) -> Annotations<T>
    where
        F: FnMut(A) -> T,
    {
        Annotations {
            nodes: self
                .nodes
                .into_iter()
                .map(|node| map(node.value).with_span(node.span))
                .collect(),
        }
    }
    pub fn filter_map<T, F>(self, mut map: F) -> Annotations<T>
    where
        F: FnMut(A) -> Option<T>,
    {
        Annotations {
            nodes: self
                .nodes
                .into_iter()
                .filter_map(|node| map(node.value).map(|v| v.with_span(node.span)))
                .collect(),
        }
    }

    pub(crate) fn map_fallible<T, E, F>(self, mut map: F, errs: &mut Vec<E>) -> Annotations<T>
    where
        F: FnMut(A) -> Result<T, E>,
    {
        Annotations {
            nodes: self
                .nodes
                .into_iter()
                .filter_map(|node| match map(node.value) {
                    Ok(res) => Some(res.with_span(node.span)),
                    Err(e) => {
                        errs.push(e);
                        None
                    }
                })
                .collect(),
        }
    }

    pub fn reduce<T, F>(&self, span: Span, initial: T, mut f: F) -> T
    where
        F: FnMut(T, &A) -> T,
    {
        let mut value = initial;
        for annotation in self.nodes.iter().rev().filter(|n| n.span.intersects(&span)) {
            value = f(value, annotation);
        }
        value
    }
}
