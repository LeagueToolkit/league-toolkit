use std::{fmt, io};

use crate::{
    bank::BankVersion, hierarchy::ObjectKind, package::PackageKey, upgrade::UpgradeRefusal,
    ObjectId, SourceId,
};

/// Returns `value` as a `u32` field, or an `InvalidInput` error that names the field `what`.
pub(crate) fn to_u32<T>(value: T, what: &str) -> io::Result<u32>
where
    T: Copy + fmt::Display,
    u32: TryFrom<T>,
{
    narrow(value, what, "u32::MAX")
}

/// Returns `value` as a `u16` field, or an `InvalidInput` error that names the field `what`.
pub(crate) fn to_u16<T>(value: T, what: &str) -> io::Result<u16>
where
    T: Copy + fmt::Display,
    u16: TryFrom<T>,
{
    narrow(value, what, "u16::MAX")
}

/// Returns `value` as a `u8` field, or an `InvalidInput` error that names the field `what`.
pub(crate) fn to_u8<T>(value: T, what: &str) -> io::Result<u8>
where
    T: Copy + fmt::Display,
    u8: TryFrom<T>,
{
    narrow(value, what, "u8::MAX")
}

/// Returns `value` as a field of the type `U`, whose maximum `max` names in the error.
fn narrow<T, U>(value: T, what: &str, max: &str) -> io::Result<U>
where
    T: Copy + fmt::Display,
    U: TryFrom<T>,
{
    U::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{what} {value} exceeds {max}"),
        )
    })
}

/// The result type of `ltk_wwise`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// An error from reading, decoding, encoding or writing a Wwise file.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),

    /// The first bytes are not the magic of the format.
    #[error(
        "invalid magic: expected {}, found {}",
        .expected.escape_ascii(),
        .found.escape_ascii()
    )]
    InvalidMagic { expected: [u8; 4], found: [u8; 4] },

    /// The version of a package is not 1.
    #[error("unsupported package version {0}")]
    PackageVersion(u32),

    /// Two entries of a package have one key.
    #[error("two package entries have the key {0}")]
    DuplicateKey(PackageKey),

    /// An entry record or a media ends after the end of the package.
    ///
    /// `name` is the entry name. An entry whose record does not fit is named `#<index>`, its
    /// index in the offset table.
    #[error("package entry {name} ends after the end of the package")]
    EntryOutOfBounds { name: String },

    /// The first chunk of a bank is not `BKHD`, or `BKHD` is shorter than its version requires.
    ///
    /// `BKHD` is 20 bytes or longer. From version 143, it is 40 bytes or longer.
    #[error("invalid bank header")]
    InvalidHeader,

    /// A chunk of a bank ends after the end of the bank.
    #[error(
        "chunk {} at offset {offset} ends after the end of the bank",
        .tag.escape_ascii()
    )]
    ChunkOverrun { tag: [u8; 4], offset: u64 },

    /// A record of `DIDX` names media that ends after the end of `DATA`.
    #[error("media {source_id} ends after the end of DATA")]
    MediaOutOfBounds { source_id: SourceId },

    /// Two records of `DIDX` have one source ID.
    #[error("two media have the source ID {0}")]
    DuplicateSource(SourceId),

    /// The objects of `HIRC` do not end at the end of the chunk.
    ///
    /// `offset` is the end of the framing that the counts and sizes give: past `len` when an
    /// object runs past the chunk, short of `len` when bytes follow the last object.
    #[error("hierarchy framing ends at {offset}, the chunk ends at {len}")]
    Framing { offset: usize, len: usize },

    /// An object payload is shorter than 4 bytes, the length of an object ID.
    #[error("object payload of {0} bytes is shorter than an ID")]
    PayloadTooShort(usize),

    /// The bank version has no payload layouts. Version 132, 134 and 145 have layouts.
    #[error("no payload layouts for bank version {0}")]
    UnsupportedLayout(BankVersion),

    /// A layout did not consume the payload exactly.
    ///
    /// `offset` is the end of the layout: past `len` when the layout needs more bytes than the
    /// payload holds, short of `len` when bytes follow the end of the layout.
    #[error("layout of {kind} {id} stops at byte {offset} of {len}")]
    Layout {
        kind: ObjectKind,
        id: ObjectId,
        offset: usize,
        len: usize,
    },

    /// An action of a class outside the table of the generation of the bank version.
    #[error("action {id} has the unknown class {class:#06x}")]
    ActionClass { id: ObjectId, class: u16 },

    /// The upgrade does not rewrite the bank.
    #[error("upgrade refused: {0}")]
    Upgrade(UpgradeRefusal),

    /// A WEM is not a RIFF `WAVE` file with a `fmt ` and a `data` chunk.
    #[error("invalid WEM: {0}")]
    Wem(&'static str),
}
