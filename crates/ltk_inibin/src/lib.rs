//! Inibin/troybin binary configuration file parser for League of Legends.
//!
//! Supports reading (v1 + v2), writing (v2), and modifying all 14 value set types.
//!
//! ```
//! use ltk_inibin::Inibin;
//!
//! let mut inibin = Inibin::new();
//! inibin.insert(0x0001, 42i32);
//! inibin.insert(0x0002, "hello");
//!
//! assert_eq!(inibin.get_as::<i32>(0x0001), Some(42));
//! assert_eq!(inibin.get_or(0x9999, 0i32), 0);
//! ```

mod error;
mod file;
mod section;
mod value;
mod value_flags;

pub use error::{Error, Result};
pub use file::Inibin;
pub use section::Section;
pub use value::{FromValue, Value};
pub use value_flags::ValueFlags;
