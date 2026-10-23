use std::fmt::Display;

use ltk_hash::{HashAlgorithm, HashValue, HashWidth, Hasher};
use ltk_meta::PropertyKind;

use crate::RitobinName as _;

/// Returns the ritobin name of a hash algorithm: `fnv1a`, `xxh64` or `xxh3`.
pub const fn hash_algorithm_name(algorithm: HashAlgorithm) -> &'static str {
    match algorithm {
        HashAlgorithm::Fnv1a32 => "fnv1a",
        HashAlgorithm::Xxh64 => "xxh64",
        HashAlgorithm::Xxh3 => "xxh3",
        _ => "unknown",
    }
}

/// Returns the hash algorithm of a ritobin name. Returns `None` if `name` is not `fnv1a`,
/// `xxh64` or `xxh3`.
pub fn hash_algorithm_from_name(name: &str) -> Option<HashAlgorithm> {
    match name {
        "fnv1a" => Some(HashAlgorithm::Fnv1a32),
        "xxh64" => Some(HashAlgorithm::Xxh64),
        "xxh3" => Some(HashAlgorithm::Xxh3),
        _ => None,
    }
}

/// Returns the ritobin name of a hash width: `u32` or `u64`.
pub const fn hash_width_name(width: HashWidth) -> &'static str {
    match width {
        HashWidth::W4 => "u32",
        HashWidth::W8 => "u64",
    }
}

/// Returns the hash width of a ritobin name. Returns `None` if `name` is not `u32` or `u64`.
pub fn hash_width_from_name(name: &str) -> Option<HashWidth> {
    match name {
        "u32" => Some(HashWidth::W4),
        "u64" => Some(HashWidth::W8),
        _ => None,
    }
}

/// Returns the hasher of the type `hash[width,algorithm]`.
///
/// The hasher lowercases the string. Every hasher in the class dumps of 16.21 lowercases the
/// string.
pub const fn rito_hasher(width: HashWidth, algorithm: HashAlgorithm) -> Hasher {
    Hasher {
        width,
        algorithm,
        lowercased: true,
    }
}

/// The type arguments of `hash`: the stored width and the hash function.
///
/// `hash` is `hash[u32,fnv1a]`. `hash[u32]` and `hash[u64]` give the width alone, for a value
/// whose hash function is not known: a hex literal takes that width, and a string is a type
/// mismatch because no function hashes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub struct HashType {
    /// The stored width.
    pub width: HashWidth,
    /// The hash function. `None` for `hash[u32]` and `hash[u64]`.
    pub algorithm: Option<HashAlgorithm>,
}

impl HashType {
    /// The type of plain `hash`: 4 bytes, FNV-1a over the lowercased string.
    pub const DEFAULT: Self = Self::new(HashWidth::W4, HashAlgorithm::Fnv1a32);

    /// Returns the type `hash[width,algorithm]`.
    pub const fn new(width: HashWidth, algorithm: HashAlgorithm) -> Self {
        Self {
            width,
            algorithm: Some(algorithm),
        }
    }

    /// Returns the type `hash[width]`, whose hash function is not known.
    pub const fn width_only(width: HashWidth) -> Self {
        Self {
            width,
            algorithm: None,
        }
    }

    /// Returns the hasher of the type. Returns `None` if the hash function is not known.
    pub const fn hasher(self) -> Option<Hasher> {
        match self.algorithm {
            Some(algorithm) => Some(rito_hasher(self.width, algorithm)),
            None => None,
        }
    }

    /// Returns `hash` at the width of the type. Keeps only the low 32 bits if the width is 4.
    pub const fn from_hash(self, hash: u64) -> HashValue {
        HashValue::with_width(hash, self.width)
    }

    /// Returns the hash of `text` at the width of the type. Returns `None` if the hash function
    /// is not known.
    pub fn hash_str(self, text: &str) -> Option<HashValue> {
        Some(self.hasher()?.hash_str(text))
    }
}

impl Default for HashType {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Writes `hash`, `hash[width]` or `hash[width,algorithm]`. Writes `hash` for
/// [`HashType::DEFAULT`].
impl Display for HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(PropertyKind::Hash.to_rito_name())?;
        if *self == Self::DEFAULT {
            return Ok(());
        }
        write!(f, "[{}", hash_width_name(self.width))?;
        if let Some(algorithm) = self.algorithm {
            write!(f, ",{}", hash_algorithm_name(algorithm))?;
        }
        f.write_str("]")
    }
}

/// Returns the type that the printer writes for a `hash` value of `width`.
///
/// A bin file does not store the hash function. Returns [`HashType::DEFAULT`] for width 4, as
/// plain `hash` is read as FNV-1a. Returns `hash[u64]` for width 8.
pub const fn hash_type_for_width(width: HashWidth) -> HashType {
    match width {
        HashWidth::W4 => HashType::DEFAULT,
        HashWidth::W8 => HashType::width_only(HashWidth::W8),
    }
}
