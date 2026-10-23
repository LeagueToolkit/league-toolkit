use crate::{
    ast::{annotation::Annotations, diagnostics::DiagnosticWithSpan, Ast},
    cst::Cst,
};

mod patch;
mod root_entry;
pub use root_entry::*;

impl Ast {
    pub fn from_cst(cst: &Cst, text: &str) -> Self {
        let ctx = Builder {
            cst,
            text,
            diagnostics: Vec::new(),
            annotations: Annotations::default(),
        };
        ctx.build()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Builder<'a> {
    pub cst: &'a Cst,
    pub text: &'a str,
    pub diagnostics: Vec<DiagnosticWithSpan>,
    pub(super) annotations: Annotations,
}

impl<'a> Builder<'a> {
    pub(super) fn cst(&self) -> &'a Cst {
        self.cst
    }

    pub(super) fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }
}
