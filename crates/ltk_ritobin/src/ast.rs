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
pub use node::{Object, Property, RootEntry, Value};
pub use to_bin::PartialBin;

use crate::{
    ast::{diagnostics::DiagnosticWithSpan, node::roots::Roots},
    Cst,
};

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;

#[derive(Debug, Clone)]
pub struct Ast {
    pub roots: Roots,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl Ast {
    pub fn root_entries(&self) -> impl Iterator<Item = &RootEntry> {
        self.roots.entries.iter().flat_map(|e| e.value.iter())
    }
}

#[derive(Debug, Clone)]
/// Something that was resolved from a value, and the value that something came from.
pub struct FromValue<T> {
    pub value: Value,
    pub resolved: T,
}

impl Cst {
    pub fn build_ast(&self, text: &str) -> crate::ast::Ast {
        crate::ast::Ast::from_cst(self, text)
    }
}
