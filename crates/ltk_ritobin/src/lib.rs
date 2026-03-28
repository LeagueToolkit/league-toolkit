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
//! type: string = "my_str"
//! version: u32 = 3
//! linked: list[string] = [ ]
//! entries: map[hash, embed] = { }
//! "#;
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
//! For resilient parsing, errors can appear embedded as nodes into the concrete syntax tree (cst), or as a list in the [`Cst`] struct. This
//! allows for more versatile behaviour for things like pretty-printing technically invalid trees,
//! since parsing will always result in a cst.
//!
//! The same handling of errors is done in the type-checker (when building a [`ltk_meta::Bin`]), to
//! always provide a best effort construction.
//!
//! ```rust
//! use ltk_ritobin::{Cst, cst::FlatErrors};
//!
//! let text = "test: u32 = 4!!2";
//! let cst = Cst::parse(text);
//! assert_eq!(cst.errors.len(), 0); // no 'top level' errors in the CST
//!
//! // helper that walks the CST, and returns all error
//! // nodes as a list
//! let flat_errors = FlatErrors::walk(&cst);
//! assert_eq!(flat_errors.len(), 1); // the unexpected "!!!" in the value
//!
//! ```

#[allow(unused)] // for doc link above
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
