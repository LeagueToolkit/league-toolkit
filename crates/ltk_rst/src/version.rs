use crate::error::RstError;

/// RST file version.
///
/// - **V2** — complex (40-bit) hashing, optional font config, mode byte.
/// - **V3** — complex (40-bit) hashing, mode byte.
/// - **V4** — simple (39-bit) hashing, mode byte.
/// - **V5** — simple (39-bit) hashing, no mode byte.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RstVersion {
    /// Version 2 — uses complex (40-bit) hashing; supports optional font config and mode byte.
    V2 = 2,
    /// Version 3 — uses complex (40-bit) hashing; has mode byte.
    V3 = 3,
    /// Version 4 — uses simple (39-bit) hashing; has mode byte.
    V4 = 4,
    /// Version 5 — uses simple (39-bit) hashing; mode byte removed.
    V5 = 5,
}

impl RstVersion {
    /// Returns the raw version number as a `u8`.
    #[inline]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Returns the [`RstHashType`] that corresponds to this version.
    pub fn hash_type(self) -> RstHashType {
        match self {
            RstVersion::V2 | RstVersion::V3 => RstHashType::Complex,
            RstVersion::V4 | RstVersion::V5 => RstHashType::Simple,
        }
    }

    /// Returns `true` if this version stores a mode byte in the file.
    pub fn has_mode_byte(self) -> bool {
        !matches!(self, RstVersion::V5)
    }

    pub(crate) fn try_from_u8(value: u8) -> Result<Self, RstError> {
        match value {
            0x02 => Ok(RstVersion::V2),
            0x03 => Ok(RstVersion::V3),
            0x04 => Ok(RstVersion::V4),
            0x05 => Ok(RstVersion::V5),
            _ => Err(RstError::UnsupportedVersion { version: value }),
        }
    }
}

/// Determines the hash bit-width used when packing a hash+offset pair into a
/// single `u64` entry in the RST hash table.
///
/// - [`Complex`](RstHashType::Complex): used by v2/v3 — 40-bit hash, offset in upper 24 bits.
/// - [`Simple`](RstHashType::Simple):  used by v4/v5 — 39-bit hash, offset in upper 25 bits.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RstHashType {
    /// 40-bit hash key (`(1 << 40) - 1`). Used by RST v2 and v3.
    Complex = 40,
    /// 39-bit hash key (`(1 << 39) - 1`). Used by RST v4 and v5.
    Simple = 39,
}

impl RstHashType {
    /// Returns the bitmask for the hash portion of a packed entry.
    #[inline]
    pub fn hash_mask(self) -> u64 {
        (1u64 << (self as u8)) - 1
    }

    /// Returns the bit-shift used when packing or unpacking the string offset.
    #[inline]
    pub fn offset_shift(self) -> u8 {
        self as u8
    }
}
