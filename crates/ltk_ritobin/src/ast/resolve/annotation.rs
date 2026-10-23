//! Builds [`Annotation`]s from the CST and attaches each to its target.

use smallvec::SmallVec;

use crate::{
    ast::{
        annotation::{Annotation, Argument},
        builder::Builder,
        diagnostics::Diagnostic as D,
        resolve::literals::{ExpectedType, ValueEvalError},
        Value,
    },
    cst::{ChildrenExt as _, Kind},
    parse::{Span, TokenKind},
    Node,
};

/// An annotation line that parsed but does not build.
///
/// Reported as [`Diagnostic::AnnotationResolve`](D::AnnotationResolve). The whole annotation is
/// dropped: nothing attaches to its target.
#[derive(Debug, Clone, Copy, thiserror::Error)]
#[non_exhaustive]
pub enum AnnotationResolveError {
    #[error("missing annotation name")]
    MissingName { span: Span },
    #[error("named argument is missing a value")]
    MissingArgValue { span: Span },
    /// `span` is the second occurrence.
    #[error("duplicate argument")]
    DuplicateArgument { span: Span },
    #[error("invalid argument value: {0}")]
    ResolveValue(#[from] ValueEvalError),
}

impl AnnotationResolveError {
    /// The offending token.
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            Self::MissingName { span }
            | Self::MissingArgValue { span }
            | Self::DuplicateArgument { span } => *span,
            Self::ResolveValue(err) => err.span(),
        }
    }
}

type Errors = SmallVec<[AnnotationResolveError; 1]>;

impl<'a> Builder<'a> {
    /// Attaches every annotation under `parent` to the sibling that follows it.
    ///
    /// One pass over the CST, before any value is resolved. A target is the next sibling that is
    /// neither an annotation nor a comment; annotations left over when `parent`'s children end
    /// are one `TrailingAnnotations` diagnostic and are dropped.
    pub(crate) fn attach_annotations(&mut self, parent: &Node) {
        let cst = self.cst;
        let mut pending: SmallVec<[Annotation; 2]> = SmallVec::new();
        for node in parent.children.get(cst).iter().filter_map(|c| c.tree(cst)) {
            match node.kind {
                Kind::Comment => {}
                Kind::Annotation => pending.extend(self.resolve_annotation(node)),
                _ => {
                    let target = node.trimmed_span(cst);
                    for annotation in pending.drain(..) {
                        self.annotations.attach(annotation, target);
                    }
                    self.attach_annotations(node);
                }
            }
        }
        if let (Some(first), Some(last)) = (pending.first(), pending.last()) {
            self.push(D::TrailingAnnotations(first.span().cover(last.span())).unwrap());
        }
    }

    /// Builds one annotation node, reporting every error it has and returning `None` on any.
    fn resolve_annotation(&mut self, node: &Node) -> Option<Annotation> {
        match self.try_resolve_annotation(node) {
            Ok(annotation) => Some(annotation),
            Err(errors) => {
                for error in errors {
                    self.push(D::AnnotationResolve(error).unwrap());
                }
                None
            }
        }
    }

    fn try_resolve_annotation(&self, node: &Node) -> Result<Annotation, Errors> {
        let cst = self.cst;
        let children = node.children.get(cst);

        let name = children
            .find_token(cst, TokenKind::Name)
            .map(|t| t.span)
            .ok_or_else(|| {
                Errors::from_elem(AnnotationResolveError::MissingName { span: node.span }, 1)
            })?;

        let mut arguments = Vec::new();
        let mut errors = Errors::new();
        if let Some(list) = children.find_tree(cst, Kind::AnnotationArgList) {
            for arg in list
                .children
                .get(cst)
                .iter()
                .filter_map(|c| c.tree(cst))
                .filter(|n| n.kind == Kind::AnnotationArg)
            {
                match self.try_resolve_arg(arg) {
                    Ok(Some(argument)) => {
                        let name = &self.text[argument.name()];
                        if arguments
                            .iter()
                            .any(|a: &Argument| &self.text[a.name()] == name)
                        {
                            errors
                                .push(AnnotationResolveError::DuplicateArgument { span: arg.span });
                        } else {
                            arguments.push(argument);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => errors.push(error),
                }
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }

        Ok(Annotation::new(node.span, name, arguments))
    }

    /// `Ok(None)` for an argument with no name token: the parser already reported it.
    fn try_resolve_arg(&self, arg: &Node) -> Result<Option<Argument>, AnnotationResolveError> {
        let cst = self.cst;
        let children = arg.children.get(cst);

        let Some(name) = children
            .first()
            .and_then(|c| c.token(cst))
            .filter(|t| t.kind == TokenKind::Name)
        else {
            return Ok(None);
        };

        let has_eq = children
            .get(1)
            .and_then(|c| c.token(cst))
            .is_some_and(|t| t.kind == TokenKind::Eq);
        if !has_eq {
            return Ok(Some(Argument::Flag(name.span)));
        }

        let token = children
            .get(2)
            .and_then(|c| c.token(cst))
            .ok_or(AnnotationResolveError::MissingArgValue { span: arg.span })?;
        let value = Value::eval(self.text, token, Some(ExpectedType::Any), None)?;
        Ok(Some(Argument::Named {
            name: name.span,
            value,
        }))
    }
}
