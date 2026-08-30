use ltk_hash::BinHash;
use miette::Diagnostic;

use super::property::Kind;
use crate::{path::PropertyPathError, BinKind};

#[derive(Debug, thiserror::Error, Diagnostic)]
#[non_exhaustive]
pub enum Error {
    #[error("Invalid file signature")]
    InvalidFileSignature,
    #[error("Invalid file version '{0}'")]
    InvalidFileVersion(u32),
    #[error("Expected a {expected} bin, found a {found} bin")]
    UnexpectedBinKind { expected: BinKind, found: BinKind },
    #[error("Invalid PTCH version '{0}' - the client only accepts 1")]
    InvalidOverrideVersion(u32),
    #[error("The PTCH declares {0} dependencies - a patch that has any cannot be loaded")]
    OverrideDependencies(u32),
    #[error("Invalid property path in patch record {index} (object {object_hash:08x}) - {source}")]
    InvalidPropertyPath {
        index: usize,
        object_hash: BinHash,
        #[source]
        source: PropertyPathError,
    },
    #[error("Invalid '{0}' - got '{1}'")]
    InvalidField(&'static str, String),
    #[error("Invalid property kind - {0}")]
    InvalidPropertyTypePrimitive(#[from] num_enum::TryFromPrimitiveError<Kind>),
    #[error("Invalid size - expected {0}, got {1} bytes")]
    InvalidSize(u64, u64),

    #[error("Container type {0:?} cannot be nested!")]
    InvalidNesting(Kind),
    #[error("Invalid map key type {0:?}, only primitive types can be used as keys.")]
    InvalidKeyType(Kind),

    #[error("Container is empty!")]
    EmptyContainer,
    #[error("Mismatched types - expected {expected:?}, got {got:?}")]
    MismatchedContainerTypes { expected: Kind, got: Kind },

    #[error(transparent)]
    ReaderError(#[from] ltk_io_ext::ReaderError),
    #[error("IO Error - {0}")]
    IOError(#[from] std::io::Error),
    #[error("UTF-8 Error - {0}")]
    Utf8Error(#[from] std::str::Utf8Error),
}
