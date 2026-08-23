pub mod build;
pub mod coerce;
pub mod diagnostics;
pub mod query;
pub mod resolve;
pub mod value;
pub mod visitor;

mod property;
mod r#struct;

mod to_bin;

#[cfg(test)]
mod tests;

pub use property::AstProperty;
pub use r#struct::AstStruct;

pub use crate::Spanned;
pub use build::{Ast, AstObject};
pub use to_bin::PartialBin;
pub use value::AstValue;

#[cfg(not(feature = "salsa"))]
pub(crate) type Ptr<T> = Box<T>;
#[cfg(feature = "salsa")]
pub(crate) type Ptr<T> = std::sync::Arc<T>;
