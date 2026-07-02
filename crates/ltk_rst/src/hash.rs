use std::fmt::LowerHex;

/// A masked RST entry hash.
///
/// This is the *truncated* xxHash of a (lowercased) key — i.e. only the lower
/// `hash_bits` bits are significant.  The number of significant bits and the
/// hash algorithm used to produce it both depend on the [`RstFormat`] the value
/// belongs to; see [`RstFormat::hash_of`].
///
/// [`RstFormat`]: crate::RstFormat
/// [`RstFormat::hash_of`]: crate::RstFormat::hash_of
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
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

impl From<RstHash> for u64 {
    fn from(value: RstHash) -> Self {
        value.0
    }
}
