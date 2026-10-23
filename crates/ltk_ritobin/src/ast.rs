pub mod annotation;
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
        annotation::{Annotations, FromAnnotation, Resolved},
        diagnostics::DiagnosticWithSpan,
        node::roots::Roots,
    },
    Cst,
};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "span_print", derive(crate::span::DebugSpans))]
pub struct Ast {
    pub roots: Roots,
    pub annotations: Annotations,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl Ast {
    pub fn root_entries(&self) -> impl Iterator<Item = &RootEntry> {
        self.roots.entries().unwrap_or_default().iter()
    }

    /// Shorthand for `self.annotations.resolve(text)`. See [`FromAnnotation`].
    #[must_use]
    pub fn resolve_annotations<T: FromAnnotation>(&self, text: &str) -> Resolved<T, T::Error> {
        self.annotations.resolve(text)
    }
}

impl Cst {
    pub fn build_ast(&self, text: &str) -> crate::ast::Ast {
        crate::ast::Ast::from_cst(self, text)
    }
}
