pub mod builder;
pub mod diagnostics;
pub mod hash;
pub mod node;
pub mod query;
pub mod resolve;
pub mod visitor;

mod to_bin;

#[cfg(test)]
mod tests;

pub use crate::Spanned;
pub use node::{Object, Property, RootEntry, RootPatch, Value};

pub use to_bin::PartialBin;

use crate::{
    ast::{
        diagnostics::DiagnosticWithSpan,
        node::{roots::Roots, Annotation, Annotations},
    },
    Cst,
};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(span::DebugSpans))]
pub struct Ast<A = Annotation> {
    pub roots: Roots,
    pub annotations: Annotations<A>,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl<A> Ast<A> {
    pub fn root_entries(&self) -> impl Iterator<Item = &RootEntry> {
        self.roots.entries().unwrap_or_default().iter()
    }

    pub fn map_annotations<T>(self, map: impl FnOnce(Annotations<A>) -> Annotations<T>) -> Ast<T> {
        Ast {
            roots: self.roots,
            annotations: map(self.annotations),
            diagnostics: self.diagnostics,
        }
    }

    pub fn try_map_annotations<T, E, F>(self, mut map: F) -> Ast<T>
    where
        E: Into<DiagnosticWithSpan>,
        F: FnMut(A) -> Result<T, E>,
    {
        let mut diagnostics = self.diagnostics;

        Ast {
            roots: self.roots,
            annotations: self
                .annotations
                .map_fallible(|a| map(a).map_err(|e| e.into()), &mut diagnostics),
            diagnostics,
        }
    }
}

impl Cst {
    pub fn build_ast(&self, text: &str) -> crate::ast::Ast {
        crate::ast::Ast::from_cst(self, text)
    }
}
