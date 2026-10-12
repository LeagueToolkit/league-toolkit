use std::{io, str::Utf8Error};

use ltk_hash::IniHash;

use crate::ValueKind;

/// An error from reading or writing an inibin file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("Unsupported inibin version {0}. The reader supports versions 1 and 2")]
    UnsupportedVersion(u8),

    #[error("The file stores key {0:08x} more than once")]
    DuplicateKey(IniHash),

    #[error(
        "The string of key {key:08x} starts at offset {offset}. The string data of the file is \
         {len} bytes long"
    )]
    StringOffsetOutOfBounds { key: IniHash, offset: u32, len: u32 },

    #[error("The string of key {key:08x} has no NUL terminator")]
    UnterminatedString { key: IniHash },

    #[error("The string of key {key:08x} is not valid UTF-8")]
    InvalidUtf8 {
        key: IniHash,
        #[source]
        source: Utf8Error,
    },

    #[error(
        "The string of key {key:08x} contains a NUL byte. A NUL byte terminates a string in the \
         file. Remove the NUL byte from the string"
    )]
    NulInString { key: IniHash },

    #[error(
        "{count} values have the storage type {kind:?}. A file can store at most 65535 values of \
         one storage type"
    )]
    TooManyValues { kind: ValueKind, count: usize },

    #[error(
        "The string data is {len} bytes long. A version 2 file can store at most 65535 bytes of \
         string data"
    )]
    StringDataTooLong { len: usize },

    #[error("IO error - {0}")]
    Io(#[from] io::Error),
}

/// A result with [`Error`] as the error type.
pub type Result<T> = std::result::Result<T, Error>;
