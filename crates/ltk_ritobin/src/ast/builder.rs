use crate::{
    ast::{diagnostics::DiagnosticWithSpan, Ast},
    cst::Cst,
};

mod root_entry;
pub use root_entry::*;

impl Ast {
    pub fn from_cst(cst: &Cst, text: &str) -> Self {
        let ctx = Builder {
            cst,
            text,
            diagnostics: Vec::new(),
        };
        ctx.build()
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
