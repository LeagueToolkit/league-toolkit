//! Bin file & properties
pub mod property;
pub use property::{value, BinProperty, Kind as PropertyKind, PropertyValueEnum};

pub mod tree;
pub use tree::{Bin, Object as BinObject};

mod error;
pub use error::*;

pub mod traits;
