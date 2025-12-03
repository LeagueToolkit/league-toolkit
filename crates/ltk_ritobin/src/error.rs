//! Error types for ritobin parsing and writing.

use thiserror::Error;

/// Errors that can occur during ritobin parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected end of input")]
    UnexpectedEof,

    #[error("invalid header: expected '#PROP_text'")]
    InvalidHeader,

    #[error("unknown type name: {0}")]
    UnknownType(String),

    #[error("invalid number: {0}")]
    InvalidNumber(String),

    #[error("invalid hex value: {0}")]
    InvalidHex(String),

    #[error("parse error at: {0}")]
    ParseError(String),

    #[error("missing type info for container type")]
    MissingTypeInfo,

    #[error("unexpected content after parsing: {0}")]
    TrailingContent(String),
}

/// Errors that can occur during ritobin writing.
#[derive(Debug, Error)]
pub enum WriteError {
    #[error("fmt error: {0}")]
    Fmt(#[from] std::fmt::Error),
}
