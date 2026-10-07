use itertools::{Either, Itertools};

use smallvec::{smallvec, SmallVec};

use crate::{
    ast::{
        builder::Builder,
        diagnostics::Diagnostic,
        node::{Annotation, Argument},
        resolve::literals::ValueEvalError,
        Value,
    },
    cst::{ChildrenExt as _, Kind as CstKind},
    escaping,
    parse::{Span, TokenKind},
    Node,
};

use super::literals::ExpectedType;

#[derive(Debug, thiserror::Error, Clone, Copy)]
pub enum AnnotationResolveError {
    #[error("Missing annotation name")]
    MissingName,
    #[error("Named annotation argument is missing value")]
    MissingArgValue,
    #[error("Annotation takes {need} arguments - got {got}")]
    InsufficientArguments { need: usize, got: usize },
    #[error("Invalid annotation argument - {0}")]
    ResolveValue(#[from] ValueEvalError),
    #[error(transparent)]
    InvalidEscape(#[from] escaping::InvalidEscape),
}

impl AnnotationResolveError {
    pub fn span(&self) -> Option<Span> {
        None
    }
}

impl<'a> Builder<'a> {
    pub fn try_resolve_annotation(
        &self,
        node: &Node,
    ) -> Result<Option<Annotation>, smallvec::SmallVec<[AnnotationResolveError; 1]>> {
        use AnnotationResolveError::*;

        let children = node.children.get(self.cst);
        let name = children
            .get(1)
            .and_then(|n| n.token(self.cst))
            .and_then(|t| matches!(t.kind, TokenKind::Name).then_some(t))
            .ok_or_else(|| smallvec![MissingName])?;

        let args = children
            .find_tree(self.cst, CstKind::AnnotationArgList)
            .map(|args| {
                (args
                    .children
                    .get(self.cst)
                    .iter()
                    .filter_map(|n| n.tree(self.cst))
                    .filter(|n| n.kind == CstKind::AnnotationArg))
                .filter_map(|n| self.try_resolve_arg(n).transpose())
            })
            .map(|args| {
                let (args, arg_errs): (Vec<Argument>, SmallVec<[_; 1]>) =
                    args.partition_map(|result| match result {
                        Ok(value) => Either::Left(value),
                        Err(error) => Either::Right(error),
                    });
                if !arg_errs.is_empty() {
                    return Err(arg_errs);
                }
                Ok(args)
            })
            .transpose()?
            .unwrap_or_default();

        Ok(Some(Annotation {
            span: node.span,
            name: name.span,
            arguments: args,
        }))
    }

    fn try_resolve_arg(&self, arg: &Node) -> Result<Option<Argument>, AnnotationResolveError> {
        let children = arg.children.get(self.cst);

        let Some(name) = children
            .first()
            .and_then(|n| n.token(self.cst))
            .filter(|t| t.kind == TokenKind::Name)
        else {
            return Ok(None);
        };

        if children
            .get(1)
            .and_then(|c| c.token(self.cst))
            .is_some_and(|tok| tok.kind == TokenKind::Eq)
        {
            let token = children
                .get(2)
                .and_then(|c| c.token(self.cst))
                .ok_or(AnnotationResolveError::MissingArgValue)?;

            let value = Value::eval(self.text, token, Some(ExpectedType::Any), None)?;
            Ok(Some(Argument::Named {
                name: name.span,
                value,
            }))
        } else {
            Ok(Some(Argument::Bare(name.span)))
        }
    }

    #[must_use]
    pub fn resolve_annotation(&mut self, node: &Node) -> Option<Annotation> {
        match self.try_resolve_annotation(node) {
            Ok(v) => v,
            Err(errs) => {
                for e in errs {
                    self.diagnostics
                        .push(Diagnostic::AnnotationResolve(e).default_span(node.span));
                }
                None
            }
        }
    }
}
