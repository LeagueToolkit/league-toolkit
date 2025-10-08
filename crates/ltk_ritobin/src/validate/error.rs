use std::num::ParseIntError;

use miette::SourceSpan;

use crate::{Span, ValueKind};

#[derive(thiserror::Error, miette::Diagnostic, Debug)]
pub enum BinError {
    #[error("Invalid root entry name")]
    #[diagnostic()]
    InvalidRootEntryName {
        #[label("Root entry name cannot be of type '{kind}'")]
        span: SourceSpan,
        kind: ValueKind,
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
