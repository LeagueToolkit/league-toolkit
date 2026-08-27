use indexmap::IndexMap;
use ltk_meta::PropertyKind;

use crate::{
    ast::{
        diagnostics::{DiagnosticWithSpan, RootKind},
        node::Value,
        root::RootKindOrUnknown,
        Ast, Ptr, Spanned,
    },
    cst::{Child, Cst, Kind, Node},
    parse::{Span, Token, TokenKind},
    RitoType,
};

mod roots;

impl Ast {
    pub fn from_cst(cst: &Cst, text: &str) -> Self {
        let mut ctx = Builder {
            cst,
            text,
            diagnostics: Vec::new(),
        };
        ctx.build_root()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Builder<'a> {
    pub cst: &'a Cst,
    pub text: &'a str,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

pub trait ChildrenExt {
    fn find_tree<'c>(&'c self, cst: &'c Cst, kind: Kind) -> Option<&'c Node>;
    fn find_token<'c>(&'c self, cst: &'c Cst, kind: TokenKind) -> Option<&'c Token>;
}

impl ChildrenExt for [Child] {
    fn find_tree<'c>(&'c self, cst: &'c Cst, kind: Kind) -> Option<&'c Node> {
        self.iter()
            .find_map(|c| c.tree(cst).filter(|t| t.kind == kind))
    }
    fn find_token<'c>(&'c self, cst: &'c Cst, kind: TokenKind) -> Option<&'c Token> {
        self.iter()
            .find_map(|c| c.token(cst).filter(|t| t.kind == kind))
    }
}

impl<'a> Builder<'a> {
    pub(super) fn cst(&self) -> &'a Cst {
        self.cst
    }

    pub(super) fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }
}
