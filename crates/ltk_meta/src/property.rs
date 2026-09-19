pub mod values;

mod kind;
pub use kind::*;

mod r#enum;
pub use r#enum::*;

mod slot;
pub use slot::ValueSlot;

use super::Error;
