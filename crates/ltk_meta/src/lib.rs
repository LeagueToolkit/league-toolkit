//! Bin file & properties
pub mod property;
pub use property::{value, BinProperty, Kind as PropertyKind, PropertyValueEnum};

mod bin_tree;
pub use bin_tree::*;

mod error;
pub use error::*;

pub mod traits;
