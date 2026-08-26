//! Ritobin text format parser and writer for League Toolkit.
//!
//! This crate provides functionality to parse and write the ritobin text format,
//! which is a human-readable representation of League of Legends bin files.
//!
//! # Quick start
//!
//! Parse ritobin text, build a [`Bin`], and round-trip it back to text:
//!
//! ```rust
//! use ltk_ritobin::{Cst, Print as _};
//!
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
//! let partial = cst.build_bin(text);
//! assert!(partial.diagnostics.is_empty());
//!
//! // Write back to text
//! let output = partial.bin.print().unwrap();
//!
//! assert_eq!(text, output);
//! ```
//!
//! # Converting `.bin` to ritobin
//!
//! `.bin` files identify fields and types by FNV-1a hashes. Supplying a [`HashProvider`]
//! lets the printer emit the original names instead of `0xdeadbeef` literals:
//!
//! ```no_run
//! use std::io::BufReader;
//! use std::fs::File;
//! use ltk_meta::Bin;
//! use ltk_ritobin::{Print, print::PrintConfig, HashMapProvider};
//!
//! let mut reader = BufReader::new(File::open("data.bin")?);
//! let bin = Bin::from_reader(&mut reader)?;
//!
//! let mut hashes = HashMapProvider::new();
//! hashes.load_from_directory("hashes/"); // loads hashes.bin{entries,fields,hashes,types}.txt
//!
//! let text = bin.print_with_config(PrintConfig::default().with_hashes(hashes))?;
//! std::fs::write("data.rito", text)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Error reporting
//!
//! For resilient parsing, errors exist as nodes into the concrete syntax tree (cst), which propagate into the [`Node`]s' `errors` field (depending on [`parse::ErrorPropagation`]). This
//! allows for more versatile behaviour with things like pretty-printing technically invalid trees,
//! since parsing will always result in a cst.
//!
//! The same handling of errors is done in the type-checker (when building a [`ltk_meta::Bin`]), to
//! always provide a best effort construction.
//!
//! ```rust
//! use ltk_ritobin::Cst;
//!
//! let text = "test: u32 = 4!!2";
//!
//! // by default uses ErrorPropagation::Move,
//! // so all errors will end up in the root
//! let cst = Cst::parse(text);
//!
//! assert_eq!(cst.errors.len(), 1); // the unexpected "!!" in the value
//! ```
//!
//! `Cst::build_bin` follows the same philosophy: it returns a [`ast::PartialBin`], pairing a
//! best-effort `Bin` with any diagnostics, so type errors don't prevent you from getting a
//! `Bin` back. This matters for editor use cases: between keystrokes a buffer is almost always
//! temporarily invalid, and tooling still needs to render it, navigate it, and report problems
//! with precise spans. Use [`ast::PartialBin::finish`] where you instead want a `Result` that
//! only succeeds on a clean build.

use std::ops::{Deref, DerefMut};

#[allow(unused, reason = "for module level doc link")]
use ltk_meta::Bin;

pub mod ast;
pub mod cst;
pub mod hashes;
pub mod parse;
pub mod print;
pub mod types;

pub use hashes::*;
pub use types::*;

pub use cst::Cst;
pub use cst::Node;
pub use print::Print;

use crate::parse::Span;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    pub fn new(span: Span, value: T) -> Self {
        Self { span, value }
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> Deref for Spanned<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> AsMut<T> for Spanned<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.value
    }
}

impl<T> AsRef<T> for Spanned<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl AsRef<str> for Spanned<String> {
    fn as_ref(&self) -> &str {
        &self.value
    }
}
