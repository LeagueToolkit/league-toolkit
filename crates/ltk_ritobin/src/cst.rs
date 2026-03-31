//! Concrete Syntax Tree (CST)
//!
//! This module defines the [`Cst`] produced by the parser, as well as the [`CstBuilder`] & the [`Visitor`] trait for walking the CST.
//! A CST is a lossless, structural representation of the source text, where every syntactic element is preserved in tree form.
//!
//! Unlike an [AST](https://en.wikipedia.org/wiki/Abstract_syntax_tree), the CST retains all tokens and their exact arrangement,
//! including delimiters and other syntax that may not carry semantic meaning.
//! This is ideal for tasks such as formatting, tooling, and any source-to-source transformations.

mod tree;
pub use tree::Kind as TreeKind;
pub use tree::*;

pub mod visitor;
pub use visitor::Visitor;

pub mod builder;
pub use builder::Builder as CstBuilder;

mod flat_errors;
pub use flat_errors::*;
