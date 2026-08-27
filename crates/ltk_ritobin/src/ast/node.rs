mod kind;
mod object;
mod property;
mod refs;
mod root_object;

pub mod value;

pub use kind::*;
pub use object::*;
pub use property::*;
pub use refs::*;
pub use root_object::*;
pub use value::*;

use crate::ast::hash::HashedLiteral;
use ltk_hash::BinHash;

pub trait NodeExt {
    #[must_use]
    fn kind(&self) -> NodeKind;

    /// This node's own class, if it's an object or struct.
    #[must_use]
    fn class_hash(&self) -> Option<HashedLiteral<BinHash>>;
}
