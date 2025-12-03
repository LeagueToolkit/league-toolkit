//! Ritobin text format parser and writer for League Toolkit.

// Nom-style parsers use elided lifetimes extensively
#![allow(mismatched_lifetime_syntaxes)]
//!
//! This crate provides functionality to parse and write the ritobin text format,
//! which is a human-readable representation of League of Legends bin files.
//!
//! # Example
//!
//! ```rust,ignore
//! use ltk_ritobin::{parse, write};
//!
//! // Parse ritobin text
//! let text = r#"
//! #PROP_text
//! type: string = "PROP"
//! version: u32 = 3
//! "#;
//!
//! let file = parse(text).unwrap();
//! let tree = file.to_bin_tree();
//!
//! // Write back to text
//! let output = write(&tree).unwrap();
//! ```

pub mod error;
pub mod parser;
pub mod types;
pub mod writer;

pub use error::{ParseError, WriteError};
pub use parser::{parse, parse_to_bin_tree, RitobinFile};
pub use types::{kind_to_type_name, type_name_to_kind, RitobinType};
pub use writer::{write, write_with_config, RitobinBuilder, WriterConfig};

