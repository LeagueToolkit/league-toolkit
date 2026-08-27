use indexmap::IndexMap;
use ltk_meta::PropertyKind;

use crate::{
    ast::{
        diagnostics::{DiagnosticWithSpan, RootKind},
        node::Value,
        root::RootKindOrUnknown,
        Ast, Ptr, Spanned,
    },
    cst::{Cst, Kind},
    parse::Span,
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

impl<'a> Builder<'a> {
    pub(super) fn cst(&self) -> &'a Cst {
        self.cst
    }

    pub(super) fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }
}
