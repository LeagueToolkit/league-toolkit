pub mod builder;
pub mod coerce;
pub mod diagnostics;
pub mod hash;
pub mod node;
pub mod query;
pub mod resolve;
pub mod root;
pub mod visitor;

mod to_bin;

#[cfg(test)]
mod tests;

pub use query::{NodeRef, SubNodeRef};

pub use crate::Spanned;
pub use node::{AstObject, AstProperty, AstStruct, AstValue};
pub use to_bin::PartialBin;

use crate::{ast::diagnostics::DiagnosticWithSpan, parse::Span, Cst};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone)]
pub struct Ast {
    pub bin_type: Option<Span>,
    pub version: Option<Spanned<u32>>,
    pub dependencies: Vec<Span>,
    pub objects: Vec<AstObject>,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl Cst {
    pub fn build_ast(&self, text: &str) -> crate::ast::Ast {
        crate::ast::Ast::from_cst(self, text)
    }
}
