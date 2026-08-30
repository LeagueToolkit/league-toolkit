pub mod values;

mod kind;
pub use kind::*;

mod r#enum;
pub use r#enum::*;

mod slot;
pub use slot::ValueSlot;

use super::Error;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoMeta;
