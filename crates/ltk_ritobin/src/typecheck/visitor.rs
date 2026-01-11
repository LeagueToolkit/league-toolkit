use indexmap::IndexMap;
use ltk_meta::{BinPropertyKind, PropertyValueEnum};

use crate::{
    parse::{
        cst::{self, visitor::Visit, Cst, Kind, Visitor},
        Span, Token, TokenKind,
    },
    typecheck::RitobinName,
};

pub struct TypeChecker<'a> {
    ctx: Ctx<'a>,
    root: IndexMap<String, PropertyValueEnum>,
    current: Option<(String, PropertyValueEnum)>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            ctx: Ctx {
                text,
                diagnostics: Vec::new(),
            },
            root: IndexMap::new(),
            current: None,
        }
    }
    pub fn into_diagnostics(self) -> Vec<DiagnosticWithSpan> {
        self.ctx.diagnostics
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Diagnostic {
    MissingTree(cst::Kind),
    EmptyTree(cst::Kind),
    MissingToken(TokenKind),
    UnknownType(Span),

    SubtypeCountMismatch {
        span: Span,
        got: u8,
        expected: u8,
    },
    /// Subtypes found on a type that has no subtypes
    UnexpectedSubtypes {
        span: Span,
        base_type: Span,
    },
}

impl Diagnostic {
    pub fn span(&self) -> Option<&Span> {
        match self {
            MissingTree(_) | EmptyTree(_) | MissingToken(_) => None,
            UnknownType(span)
            | SubtypeCountMismatch { span, .. }
            | UnexpectedSubtypes { span, .. } => Some(span),
        }
    }

    pub fn default_span(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap_or(span),
            diagnostic: self,
        }
    }

    pub fn unwrap(self) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span().copied().unwrap(),
            diagnostic: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiagnosticWithSpan {
    pub diagnostic: Diagnostic,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub struct MaybeSpanDiag {
    pub diagnostic: Diagnostic,
    pub span: Option<Span>,
}

impl MaybeSpanDiag {
    pub fn fallback(self, span: Span) -> DiagnosticWithSpan {
        DiagnosticWithSpan {
            span: self.span.unwrap_or(span),
            diagnostic: self.diagnostic,
        }
    }
}

impl From<Diagnostic> for MaybeSpanDiag {
    fn from(diagnostic: Diagnostic) -> Self {
        Self {
            span: diagnostic.span().copied(),
            diagnostic,
        }
    }
}

use Diagnostic::*;

#[derive(Debug, Clone, Copy)]
pub struct RitoType {
    pub base: BinPropertyKind,
}

pub enum Statement {
    KeyValue {
        key: Span,
        value: Span,
        kind: Option<RitoType>,
    },
}

trait TreeIterExt<'a>: Iterator {
    fn expect_tree(&mut self, kind: cst::Kind) -> Result<&'a Cst, Diagnostic>;
    fn expect_token(&mut self, kind: TokenKind) -> Result<&'a Token, Diagnostic>;
}

impl<'a, I> TreeIterExt<'a> for I
where
    I: Iterator<Item = &'a cst::Child>,
{
    fn expect_tree(&mut self, kind: cst::Kind) -> Result<&'a Cst, Diagnostic> {
        self.find_map(|c| c.tree().filter(|t| t.kind == kind))
            .ok_or(MissingTree(kind))
    }
    fn expect_token(&mut self, kind: TokenKind) -> Result<&'a Token, Diagnostic> {
        self.find_map(|c| c.token().filter(|t| t.kind == kind))
            .ok_or(MissingToken(kind))
    }
}

pub struct Ctx<'a> {
    text: &'a str,
    diagnostics: Vec<DiagnosticWithSpan>,
}

pub fn resolve_rito_type(ctx: &mut Ctx<'_>, tree: &Cst) -> Result<RitoType, Diagnostic> {
    let mut c = tree.children.iter();

    let base = c.expect_token(TokenKind::Name)?;
    let base_span = base.span;

    let base =
        BinPropertyKind::from_ritobin_name(&ctx.text[base.span]).ok_or(UnknownType(base.span))?;

    if let Some(subtypes) = c
        .clone()
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeArgList))
    {
        let subtypes_span = subtypes.span;

        let expected = base.subtype_count();

        if expected == 0 {
            return Err(UnexpectedSubtypes {
                span: subtypes_span,
                base_type: base_span,
            });
        }

        let subtypes = subtypes
            .children
            .iter()
            .filter_map(|c| c.tree().filter(|t| t.kind == Kind::TypeArg))
            .map(|t| {
                let resolved = BinPropertyKind::from_ritobin_name(&ctx.text[t.span]);
                if resolved.is_none() {
                    ctx.diagnostics.push(UnknownType(t.span).unwrap());
                }
                (resolved, t.span)
            })
            .collect::<Vec<_>>();

        if subtypes.len() > expected.into() {
            return Err(SubtypeCountMismatch {
                span: subtypes[expected as _..]
                    .iter()
                    .map(|s| s.1)
                    .reduce(|acc, s| Span::new(acc.start, s.end))
                    .unwrap_or(subtypes_span),
                got: subtypes.len() as u8,
                expected,
            });
        }
        if subtypes.len() < expected.into() {
            return Err(SubtypeCountMismatch {
                span: subtypes.last().map(|s| s.1).unwrap_or(subtypes_span),
                got: subtypes.len() as u8,
                expected,
            });
        }
    }

    Ok(RitoType { base })
}

pub fn resolve_entry(
    ctx: &mut Ctx,
    tree: &Cst,
) -> Result<(Span, Option<RitoType>, Span), MaybeSpanDiag> {
    let mut c = tree.children.iter();

    let key = c.expect_tree(Kind::EntryKey)?;

    let kind = c
        .clone()
        .find_map(|c| c.tree().filter(|t| t.kind == Kind::TypeExpr))
        .map(|t| resolve_rito_type(ctx, t))
        .transpose()?;

    let value = c.expect_tree(Kind::EntryValue)?;

    Ok((key.span, kind, value.span))
}

pub fn resolve_list(ctx: &mut Ctx, tree: &Cst) -> Result<(), Diagnostic> {
    Ok(())
}

impl Visitor for TypeChecker<'_> {
    fn enter_tree(&mut self, tree: &Cst) -> Visit {
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Skip;
        }

        match self.current.as_mut() {
            Some((name, value)) => {}
            None => {
                match tree.kind {
                    Kind::Entry => {}
                    Kind::File => return Visit::Continue,
                    _ => return Visit::Skip,
                }

                match resolve_entry(&mut self.ctx, tree).map_err(|e| e.fallback(tree.span)) {
                    Ok(entry) => {
                        eprintln!("entry: {entry:?}");
                    }
                    Err(e) => self.ctx.diagnostics.push(e),
                }
            }
        }

        Visit::Continue
    }

    fn exit_tree(&mut self, tree: &cst::Cst) -> Visit {
        if tree.kind == cst::Kind::ErrorTree {
            return Visit::Continue;
        }

        match self.current.take() {
            Some((name, value)) => {
                // TODO: warn when shadowed
                let _existing = self.root.insert(name, value);
            }
            None => {
                eprintln!("exit tree with no current?");
            }
        }

        Visit::Continue
    }
}
