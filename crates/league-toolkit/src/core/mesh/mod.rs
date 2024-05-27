mod r#static;

use error::ParseError;
pub use r#static::*;

pub mod error;
pub use error::*;

pub mod skinned;
pub use skinned::*;

pub type Result<T> = core::result::Result<T, ParseError>;
