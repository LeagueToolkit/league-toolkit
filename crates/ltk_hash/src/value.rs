//! Provides the value type of a bin `Hash` property and the hasher that computes the value.
//!
//! A bin file stores a `Hash` property value in 4 or 8 bytes. The client keeps the hash in a
//! `u64`. Each `Hash` property has a helper that defines the stored size and the hash function.
//! [`HashValue`] is the stored value with its size. [`Hasher`] is the helper.

use std::fmt::{self, Display, LowerHex};

use xxhash_rust::{xxh3::xxh3_64, xxh64::xxh64};

use crate::{fnv1a, BinHash};

/// The number of bytes that a `Hash` value occupies in a file.
#[repr(u8)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub enum HashWidth {
    /// 4 bytes.
    #[default]
    W4 = 4,
    /// 8 bytes.
    W8 = 8,
}

impl HashWidth {
    /// Returns the width in bytes.
    #[inline]
    #[must_use]
    pub const fn bytes(self) -> usize {
        self as usize
    }

    /// Returns the width of `bytes` bytes. Returns `None` if `bytes` is not 4 or 8.
    #[inline]
    #[must_use]
    pub const fn from_bytes(bytes: usize) -> Option<Self> {
        match bytes {
            4 => Some(Self::W4),
            8 => Some(Self::W8),
            _ => None,
        }
    }
}

/// The value of a `Hash` property: the hash, zero-extended to 64 bits, and its stored width.
///
/// Two values with the same hash and different widths are not equal. The writer writes them as
/// different bytes.
///
/// A value of width 4 is at most `u32::MAX`. Every constructor enforces the limit.
///
/// # Examples
///
/// ```
/// use ltk_hash::{HashValue, HashWidth};
///
/// let narrow = HashValue::narrow(0x8d39_bde6);
/// assert_eq!(narrow.width(), HashWidth::W4);
/// assert_eq!(narrow.to_string(), "8d39bde6");
///
/// let wide = HashValue::wide(0x8d39_bde6);
/// assert_eq!(wide.as_u32(), 0x8d39_bde6);
/// assert_eq!(wide.to_string(), "000000008d39bde6");
/// assert_ne!(narrow, wide);
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct HashValue {
    hash: u64,
    width: HashWidth,
}

impl HashValue {
    /// Returns a value of width 4.
    #[inline]
    #[must_use]
    pub const fn narrow(hash: u32) -> Self {
        Self {
            hash: hash as u64,
            width: HashWidth::W4,
        }
    }

    /// Returns a value of width 8.
    #[inline]
    #[must_use]
    pub const fn wide(hash: u64) -> Self {
        Self {
            hash,
            width: HashWidth::W8,
        }
    }

    /// Returns a value of `width`. Keeps only the low 32 bits of `hash` if `width` is
    /// [`HashWidth::W4`].
    #[inline]
    #[must_use]
    pub const fn with_width(hash: u64, width: HashWidth) -> Self {
        match width {
            HashWidth::W4 => Self::narrow(hash as u32),
            HashWidth::W8 => Self::wide(hash),
        }
    }

    /// Returns a value of `width`. Returns `None` if `width` is [`HashWidth::W4`] and `hash`
    /// is above `u32::MAX`.
    #[inline]
    #[must_use]
    pub const fn try_with_width(hash: u64, width: HashWidth) -> Option<Self> {
        match width {
            HashWidth::W4 if hash > u32::MAX as u64 => None,
            _ => Some(Self::with_width(hash, width)),
        }
    }

    /// Returns the hash, zero-extended to 64 bits.
    #[inline]
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.hash
    }

    /// Returns the low 32 bits of the hash, at either width.
    #[inline]
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.hash as u32
    }

    /// Returns the hash as `u32` if the width is 4. Returns `None` if the width is 8.
    #[inline]
    #[must_use]
    pub const fn try_as_u32(self) -> Option<u32> {
        match self.width {
            HashWidth::W4 => Some(self.hash as u32),
            HashWidth::W8 => None,
        }
    }

    /// Returns the hash as a [`BinHash`] if the width is 4. Returns `None` if the width is 8.
    #[inline]
    #[must_use]
    pub const fn try_as_bin_hash(self) -> Option<BinHash> {
        match self.try_as_u32() {
            Some(hash) => Some(BinHash(hash)),
            None => None,
        }
    }

    /// Returns the stored width.
    #[inline]
    #[must_use]
    pub const fn width(self) -> HashWidth {
        self.width
    }
}

impl From<u32> for HashValue {
    fn from(hash: u32) -> Self {
        Self::narrow(hash)
    }
}

impl From<BinHash> for HashValue {
    fn from(hash: BinHash) -> Self {
        Self::narrow(hash.0)
    }
}

/// Returns the FNV-1a 32 hash of the lowercased string at width 4. `BinHash::from` returns the
/// same hash.
impl From<&str> for HashValue {
    fn from(text: &str) -> Self {
        Hasher::DEFAULT.hash_str(text)
    }
}

/// Writes the hash in lowercase hex, zero-padded to 8 digits at width 4 and to 16 digits at
/// width 8.
impl Display for HashValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        LowerHex::fmt(self, f)
    }
}

/// Writes the same text as the [`Display`] implementation. Ignores the width, the fill and the
/// flags of the formatter.
impl LowerHex for HashValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.width {
            HashWidth::W4 => write!(f, "{:08x}", self.hash),
            HashWidth::W8 => write!(f, "{:016x}", self.hash),
        }
    }
}

/// The hash function of a [`Hasher`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub enum HashAlgorithm {
    /// FNV-1a with a 32-bit state.
    Fnv1a32,
    /// XXH64 with seed 0.
    Xxh64,
    /// XXH3 with a 64-bit result and seed 0.
    Xxh3,
}

/// The hash function and the stored width of one `Hash` property, as the helper of the property
/// in the client defines them.
///
/// A bin file does not store the hasher of a property. A class dump contains it.
///
/// # Examples
///
/// ```
/// use ltk_hash::{BinHash, Hash as _, HashAlgorithm, HashWidth, Hasher};
///
/// let name = "Characters/Zac/Skins/Skin31/Materials/Body";
/// assert_eq!(Hasher::DEFAULT.hash_str(name), BinHash::hash_str(name).into());
///
/// let hasher = Hasher {
///     width: HashWidth::W8,
///     algorithm: HashAlgorithm::Xxh3,
///     lowercased: true,
/// };
/// assert_eq!(hasher.hash_str(name).width(), HashWidth::W8);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub struct Hasher {
    /// The stored width of the property.
    pub width: HashWidth,
    /// The hash function.
    pub algorithm: HashAlgorithm,
    /// `true` if [`Hasher::hash_str`] lowercases the string before it hashes the string.
    /// [`Hasher::hash_str`] lowercases only ASCII letters for [`HashAlgorithm::Xxh64`] and
    /// [`HashAlgorithm::Xxh3`].
    pub lowercased: bool,
}

impl Hasher {
    /// The default hasher: FNV-1a 32 over the lowercased string, stored in 4 bytes.
    /// [`BinHash::hash_str`](crate::Hash::hash_str) computes the same hash.
    pub const DEFAULT: Self = Self {
        width: HashWidth::W4,
        algorithm: HashAlgorithm::Fnv1a32,
        lowercased: true,
    };

    /// Returns the hash of `text` at the width of the hasher.
    ///
    /// Returns the low 32 bits of a 64-bit hash if the width is 4.
    #[must_use]
    pub fn hash_str(&self, text: &str) -> HashValue {
        let hash = match (self.algorithm, self.lowercased) {
            (HashAlgorithm::Fnv1a32, true) => u64::from(fnv1a::hash_lower(text)),
            (HashAlgorithm::Fnv1a32, false) => u64::from(fnv1a::hash(text)),
            (HashAlgorithm::Xxh64, true) => xxh64(text.to_ascii_lowercase().as_bytes(), 0),
            (HashAlgorithm::Xxh64, false) => xxh64(text.as_bytes(), 0),
            (HashAlgorithm::Xxh3, true) => xxh3_64(text.to_ascii_lowercase().as_bytes()),
            (HashAlgorithm::Xxh3, false) => xxh3_64(text.as_bytes()),
        };
        self.from_hash(hash)
    }

    /// Returns `hash` at the width of the hasher. Keeps only the low 32 bits if the width is 4.
    #[must_use]
    pub const fn from_hash(&self, hash: u64) -> HashValue {
        HashValue::with_width(hash, self.width)
    }
}

impl Default for Hasher {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(feature = "serde")]
mod serde_impl {
    //! Implements `Serialize` and `Deserialize` for [`HashValue`].
    //!
    //! In a human-readable format, a value of width 4 is a `u32`, as a [`BinHash`](crate::BinHash)
    //! is. A value of width 8 is the string `0x` followed by 16 hex digits. A JavaScript reader
    //! loses the low bits of a JSON number above 2^53.
    //!
    //! In a binary format, a value is the tuple of the hash and the width in bytes.

    use serde::{
        de::{self, Unexpected, Visitor},
        Deserialize, Deserializer, Serialize, Serializer,
    };

    use super::{HashValue, HashWidth};

    /// The newtype name that the serializer receives. [`BinHash`](crate::BinHash) serializes with
    /// the same name.
    const NAME: &str = "BinHash";

    impl Serialize for HashValue {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            if !serializer.is_human_readable() {
                return (self.hash, self.width as u8).serialize(serializer);
            }
            match self.width {
                HashWidth::W4 => serializer.serialize_newtype_struct(NAME, &(self.hash as u32)),
                HashWidth::W8 => {
                    serializer.serialize_newtype_struct(NAME, &format!("0x{:016x}", self.hash))
                }
            }
        }
    }

    impl<'de> Deserialize<'de> for HashValue {
        fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            if !deserializer.is_human_readable() {
                let (hash, width) = <(u64, u8)>::deserialize(deserializer)?;
                let width = HashWidth::from_bytes(usize::from(width)).ok_or_else(|| {
                    de::Error::invalid_value(Unexpected::Unsigned(u64::from(width)), &"4 or 8")
                })?;
                return HashValue::try_with_width(hash, width).ok_or_else(|| {
                    de::Error::invalid_value(Unexpected::Unsigned(hash), &"a 32-bit hash")
                });
            }
            deserializer.deserialize_newtype_struct(NAME, HumanReadable)
        }
    }

    struct HumanReadable;

    impl<'de> Visitor<'de> for HumanReadable {
        type Value = HashValue;

        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a 32-bit hash as a number, or a 64-bit hash as `0x` and 16 hex digits")
        }

        fn visit_newtype_struct<D: Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(self)
        }

        fn visit_u64<E: de::Error>(self, hash: u64) -> Result<Self::Value, E> {
            u32::try_from(hash)
                .map(HashValue::narrow)
                .map_err(|_| E::invalid_value(Unexpected::Unsigned(hash), &self))
        }

        fn visit_i64<E: de::Error>(self, hash: i64) -> Result<Self::Value, E> {
            u32::try_from(hash)
                .map(HashValue::narrow)
                .map_err(|_| E::invalid_value(Unexpected::Signed(hash), &self))
        }

        fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
            text.strip_prefix("0x")
                .filter(|digits| {
                    digits.len() == 16 && digits.bytes().all(|b| b.is_ascii_hexdigit())
                })
                .and_then(|digits| u64::from_str_radix(digits, 16).ok())
                .map(HashValue::wide)
                .ok_or_else(|| E::invalid_value(Unexpected::Str(text), &self))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Hash as _;

    #[test]
    fn with_width_keeps_low_32_bits_at_width_4() {
        let value = HashValue::with_width(0x1122_3344_5566_7788, HashWidth::W4);
        assert_eq!(value, HashValue::narrow(0x5566_7788));
        assert_eq!(value.as_u64(), 0x5566_7788);
    }

    #[test]
    fn try_with_width_returns_none_above_u32_max_at_width_4() {
        assert_eq!(HashValue::try_with_width(1 << 32, HashWidth::W4), None);
        assert_eq!(
            HashValue::try_with_width(1 << 32, HashWidth::W8),
            Some(HashValue::wide(1 << 32))
        );
        assert_eq!(
            HashValue::try_with_width(7, HashWidth::W4),
            Some(HashValue::narrow(7))
        );
    }

    #[test]
    fn as_u32_returns_low_32_bits_at_width_8() {
        let value = HashValue::wide(0x1122_3344_5566_7788);
        assert_eq!(value.as_u32(), 0x5566_7788);
        assert_eq!(value.try_as_u32(), None);
        assert_eq!(value.try_as_bin_hash(), None);
        assert_eq!(HashValue::narrow(9).try_as_u32(), Some(9));
    }

    #[test]
    fn eq_compares_width() {
        assert_ne!(HashValue::narrow(0), HashValue::wide(0));
        assert_eq!(HashValue::default(), HashValue::narrow(0));
        assert_eq!(HashValue::narrow(5), HashValue::from(BinHash(5)));
        assert_eq!(HashValue::narrow(5).try_as_bin_hash(), Some(BinHash(5)));
    }

    #[test]
    fn display_pads_to_width() {
        assert_eq!(HashValue::narrow(0x1a).to_string(), "0000001a");
        assert_eq!(HashValue::wide(0x1a).to_string(), "000000000000001a");
        assert_eq!(format!("{:x}", HashValue::narrow(0x1a)), "0000001a");
    }

    #[test]
    fn hasher_default_matches_bin_hash() {
        for text in ["", "Test", "Characters/Zac", "\u{c9}t\u{e9}"] {
            assert_eq!(
                Hasher::DEFAULT.hash_str(text),
                HashValue::from(BinHash::hash_str(text))
            );
        }
    }

    #[test]
    fn hasher_lowercases_when_lowercased() {
        let lowercased = Hasher {
            width: HashWidth::W8,
            algorithm: HashAlgorithm::Xxh3,
            lowercased: true,
        };
        let exact = Hasher {
            lowercased: false,
            ..lowercased
        };

        assert_eq!(lowercased.hash_str("ABC"), lowercased.hash_str("abc"));
        assert_eq!(exact.hash_str("abc"), lowercased.hash_str("abc"));
        assert_ne!(exact.hash_str("ABC"), exact.hash_str("abc"));
        assert_eq!(exact.hash_str("abc").as_u64(), xxh3_64(b"abc"));
    }

    /// The expected hash is the `name` value of a `StaticMaterialDef` in a 16.21 skin bin. The
    /// input is the path of that object.
    #[test]
    fn hasher_xxh3_matches_shipped_material_name() {
        let hasher = Hasher {
            width: HashWidth::W8,
            algorithm: HashAlgorithm::Xxh3,
            lowercased: true,
        };
        assert_eq!(
            hasher.hash_str("Characters/Zac/Skins/Skin31/Materials/ult"),
            HashValue::wide(0x5d07_ca0d_22ff_9588)
        );
    }

    #[test]
    fn hasher_keeps_low_32_bits_at_width_4() {
        let hasher = Hasher {
            width: HashWidth::W4,
            algorithm: HashAlgorithm::Xxh64,
            lowercased: false,
        };
        let full = xxh64(b"abc", 0);
        assert_eq!(hasher.hash_str("abc"), HashValue::narrow(full as u32));
    }
}
