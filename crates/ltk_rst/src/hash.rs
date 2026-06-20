use std::fmt::LowerHex;

use xxhash_rust::xxh64::xxh64;

use crate::version::RstHashType;
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct RstHash(pub u64);

impl LowerHex for RstHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::ops::Deref for RstHash {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<u64> for RstHash {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl RstHash {
    /// Computes the masked XXHash64 of `key` (lowercased), suitable for use
    /// as an RST entry hash (without the string-offset component).
    ///
    /// The result is masked as defined by the [`RstHashType`]
    #[must_use]
    #[inline(always)]
    pub fn new(key: impl AsRef<str>, hash_type: RstHashType) -> Self {
        let lowered = key.as_ref().to_ascii_lowercase();
        let raw = xxh64(lowered.as_bytes(), 0);
        Self(raw & hash_type.hash_mask())
    }

    #[must_use]
    #[inline(always)]
    pub fn pack_entry(self, offset: u64, hash_type: RstHashType) -> PackedHash {
        PackedHash::pack(self, offset, hash_type)
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct PackedHash(pub u64);

impl LowerHex for PackedHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<u64> for PackedHash {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl std::ops::Deref for PackedHash {
    type Target = u64;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// TODO: make hash_type mismatching impossible via type system
impl PackedHash {
    /// Packs a pre-computed masked `hash` together with a string `offset` into the
    /// single `u64` value, for use in the RST hash table.
    ///
    /// NOTE: hash_type MUST match what was given when creating your [`RstHash`]
    #[must_use]
    #[inline(always)]
    pub fn pack(hash: RstHash, offset: u64, hash_type: RstHashType) -> Self {
        Self(hash.0 | (offset << hash_type.offset_shift()))
    }

    /// Unpacks a raw RST hash-table entry into `(hash, offset)`.
    #[must_use]
    #[inline(always)]
    pub fn unpack_entry(self, hash_type: RstHashType) -> (RstHash, u64) {
        let hash = self.0 & hash_type.hash_mask();
        let offset = self.0 >> hash_type.offset_shift();
        (RstHash(hash), offset)
    }
}
