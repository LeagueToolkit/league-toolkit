pub mod values;

mod kind;
pub use kind::*;

mod r#enum;
pub use r#enum::*;

use super::Error;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoMeta;
