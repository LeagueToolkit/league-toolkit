use std::num::ParseIntError;

use miette::SourceSpan;

use crate::parse::{literals::LiteralKind, Span};

#[derive(thiserror::Error, miette::Diagnostic, Debug)]
pub enum BinError {
    #[error("Invalid root entry name")]
    #[diagnostic()]
    InvalidRootEntryName {
        #[label("Root entry name cannot be of type '{kind}'")]
        span: SourceSpan,
        kind: LiteralKind,
        #[help]
        help: Option<&'static str>,
    },
    #[error("Invalid hash - {inner}")]
    #[diagnostic()]
    InvalidHash {
        #[label]
        span: SourceSpan,
        inner: ParseIntError,
    },
    #[error("Type mismatch")]
    #[diagnostic()]
    TypeMismatch {
        #[label]
        type_span: SourceSpan,
    },
    #[error("Missing type definition")]
    #[diagnostic()]
    RootTypeMissing {
        #[label("Root entries must have type definitions")]
        span: SourceSpan,
    },

    #[error("Unknown type '{value}'")]
    #[diagnostic()]
    UnknownType {
        #[label]
        span: SourceSpan,
        value: String,
    },

    #[error("Insufficient type arguments")]
    #[diagnostic()]
    InsufficientTypeArguments {
        #[label("got {got}, need {need} arguments")]
        span: SourceSpan,
        got: usize,
        need: usize,
    },
    #[error("Too many type arguments")]
    #[diagnostic()]
    TooManyTypeArguments {
        #[label("extraneous type arguments")]
        span: SourceSpan,
        need: usize,
    },
}

#[derive(thiserror::Error, miette::Diagnostic, Debug)]
#[error("Failed to process ritobin")]
#[diagnostic()]
pub struct MultiBinError {
    #[source_code]
    pub source_code: String,
    #[related]
    pub related: Vec<BinError>,
}

pub trait ToMietteSpan {
    fn into_miette(self) -> miette::SourceSpan;
}

impl ToMietteSpan for Span<'_> {
    fn into_miette(self) -> miette::SourceSpan {
        miette::SourceSpan::new(self.location_offset().into(), self.len())
    }
}
