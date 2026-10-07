use crate::{
    ast::{
        diagnostics::DiagnosticWithSpan,
        node::{Annotation, Annotations},
        Ast,
    },
    cst::Cst,
};

mod patch;
mod root_entry;
pub use root_entry::*;
use span::Span;

impl Ast {
    pub fn from_cst(cst: &Cst, text: &str) -> Self {
        let ctx = Builder {
            cst,
            text,
            diagnostics: Vec::new(),
            annotations: Annotations::new(),
        };
        ctx.build()
    }
}

#[derive(Debug, Clone)]
pub(super) struct Builder<'a> {
    pub cst: &'a Cst,
    pub text: &'a str,
    pub diagnostics: Vec<DiagnosticWithSpan>,
    annotations: Annotations,
}

impl<'a> Builder<'a> {
    pub(super) fn cst(&self) -> &'a Cst {
        self.cst
    }

    pub(super) fn push(&mut self, d: DiagnosticWithSpan) {
        self.diagnostics.push(d);
    }

    #[allow(unused, reason = "alan: I expect to need this soon enough")]
    pub(super) fn handle_err<T, E: Into<DiagnosticWithSpan>>(
        &mut self,
        result: Result<T, E>,
    ) -> Option<T> {
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                self.push(e.into());
                None
            }
        }
    }

    pub(super) fn apply_annotations(
        &mut self,
        annotations: impl IntoIterator<Item = Annotation>,
        target: Span,
    ) {
        self.annotations.extend(annotations, target)
    }
}
