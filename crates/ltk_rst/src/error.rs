use std::io;

use thiserror::Error;

use crate::version::{HashBits, RstHashAlgo, RstVersion};

#[derive(Error, Debug)]
pub enum RstError {
    #[error("invalid magic code (expected [0x52, 0x53, 0x54], got {actual:?})")]
    InvalidMagic { actual: [u8; 3] },

    #[error("unsupported RST version: {version:#04x}")]
    UnsupportedVersion { version: u8 },

    #[error("invalid hash bit-width {bits}: must be in 1..=63")]
    InvalidHashBits { bits: u8 },

    #[error(
        "could not determine the hash bit-width for RST v{version} from the data; \
         supply it explicitly via `Stringtable::reader().hash_bits(..)`"
    )]
    IndeterminateHashBits { version: u8 },

    #[error("string offset {offset} does not fit in {} bits", 64 - hash_bits.get())]
    OffsetOverflow { offset: u64, hash_bits: HashBits },

    #[error("string offset {offset} points past the end of the data section")]
    InvalidOffset { offset: u64 },

    #[error(
        "cannot retarget from {from:?} to {to:?}: stored hashes can't be re-hashed \
         without the original keys"
    )]
    IncompatibleAlgo { from: RstHashAlgo, to: RstHashAlgo },

    #[error(
        "cannot retarget from {from} to {to} hash bits: the high bits were masked \
         off and can't be recovered"
    )]
    CannotWiden { from: HashBits, to: HashBits },

    #[error("io error")]
    IoError(#[from] io::Error),
}

impl From<num_enum::TryFromPrimitiveError<RstVersion>> for RstError {
    fn from(err: num_enum::TryFromPrimitiveError<RstVersion>) -> Self {
        RstError::UnsupportedVersion {
            version: err.number,
        }
    }
}
