//! Error types for ritobin parsing and writing.

// The miette Diagnostic derive macro generates code that triggers this warning
#![allow(unused_assignments)]

use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

/// Errors that can occur during ritobin writing.
#[derive(Debug, Error, Diagnostic)]
pub enum WriteError {
    #[error("fmt error: {0}")]
    #[diagnostic(code(ltk_ritobin::write::fmt))]
    Fmt(#[from] std::fmt::Error),
}
