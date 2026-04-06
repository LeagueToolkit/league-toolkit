//! Ritobin text format parser and writer for League Toolkit.
//!
//! This crate provides functionality to parse and write the ritobin text format,
//! which is a human-readable representation of League of Legends bin files.
//!
//! # Example
//!
//! ```rust
//! use ltk_ritobin::{Cst, Print as _};
//!
//! // Parse ritobin text
//! let text = r#"
//! #PROP_text
//! type: string = "PROP"
//! version: u32 = 3
//! linked: list[string] = { }
//! entries: map[hash, embed] = { }
//! "#.trim();
//!
//! let cst = Cst::parse(text);
//! assert!(cst.errors.is_empty());
//!
//! let (bin, bin_errors) = cst.build_bin(text);
//! assert!(bin_errors.is_empty());
//!
//! // Write back to text
//! let output = bin.print().unwrap();
//!
//! assert_eq!(text, output);
//! ```
//!
//! # Error Reporting
//!
//! For resilient parsing, errors exist as nodes into the concrete syntax tree (cst), which propagate into the [`Cst`] nodes' `errors` field (depending on [`parse::ErrorPropagation`]. This
//! allows for more versatile behaviour with things like pretty-printing technically invalid trees,
//! since parsing will always result in a cst.
//!
//! The same handling of errors is done in the type-checker (when building a [`ltk_meta::Bin`]), to
//! always provide a best effort construction.
//!
//! ```rust
//! use ltk_ritobin::{Cst};
//!
//! let text = "test: u32 = 4!!2";
//!
//! // by default uses ErrorPropagation::Move,
//! // so all errors will end up in the root
//! let cst = Cst::parse(text);
//!
//! assert_eq!(cst.errors.len(), 1); // the unexpected "!!" in the value
//!
//! ```

#[allow(unused, reason = "for module level doc link")]
use ltk_meta::Bin;

pub mod cst;
pub mod hashes;
pub mod parse;
pub mod print;
pub mod typecheck;
pub mod types;

pub use hashes::*;
pub use types::*;

pub use cst::Cst;
pub use print::Print;
