use num_enum::{IntoPrimitive, TryFromPrimitive};
use xxhash_rust::{xxh3, xxh64};

use crate::error::RstError;
use crate::hash::RstHash;

/// RST file-format version, taken from the byte after the `RST` magic.
///
/// The version byte controls the *structural* framing of the file (whether a
/// font-config block and/or a trailing encryption-flag byte are present).  It does
/// **not**, on its own, determine the hash algorithm or the hash/offset split —
/// those depend on the game patch the file was produced for and are captured
/// separately in [`RstFormat`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum RstVersion {
    /// Version 2 — optional font-config block, trailing encryption-flag byte.
    V2 = 2,
    /// Version 3 — trailing encryption-flag byte.
    V3 = 3,
    /// Version 4 — trailing encryption-flag byte.
    V4 = 4,
    /// Version 5 — no font config, no trailing byte.
    V5 = 5,
}

impl RstVersion {
    /// The latest version this crate writes by default.
    pub const LATEST: RstVersion = RstVersion::V5;

    /// Whether this version carries an optional font-config block right after
    /// the version byte (only ever present in V2).
    #[inline]
    pub fn has_font_config(self) -> bool {
        matches!(self, RstVersion::V2)
    }

    /// Whether this version stores a trailing encryption-flag byte after the entry
    /// table (V2–V4; removed in V5).
    ///
    /// The byte flags whether the file contains encrypted entries (the `trenc`
    /// flag in CDTB / cdragon).
    #[inline]
    pub fn has_encryption_flag(self) -> bool {
        !matches!(self, RstVersion::V5)
    }

    /// The hash/offset split (`hash_bits`) implied purely by the version byte,
    /// for versions where it is unambiguous.
    ///
    /// V2/V3 are always 40-bit.  V4/V5 are ambiguous (39-bit before patch 15.2,
    /// 38-bit since) and therefore return `None` — they must be auto-detected
    /// from the data or supplied explicitly.
    #[inline]
    pub fn fixed_hash_bits(self) -> Option<HashBits> {
        match self {
            RstVersion::V2 | RstVersion::V3 => Some(HashBits::B40),
            RstVersion::V4 | RstVersion::V5 => None,
        }
    }
}

/// A validated hash bit-width: the number of low bits of a packed entry that
/// hold the hash (the rest hold the string offset).
///
/// Always in `1..=63`, so the shifts in [`pack`](RstFormat::pack) /
/// [`unpack`](RstFormat::unpack) / [`hash_mask`](RstFormat::hash_mask) can
/// never overflow.  Real files only ever use [`B40`](Self::B40) (V2/V3),
/// [`B39`](Self::B39), or [`B38`](Self::B38) (V4/V5).
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HashBits(u8);

impl HashBits {
    /// 40-bit split — V2/V3 files.
    pub const B40: HashBits = HashBits(40);
    /// 39-bit split — V4/V5 files before game patch 15.2.
    pub const B39: HashBits = HashBits(39);
    /// 38-bit split — V4/V5 files since game patch 15.2.
    pub const B38: HashBits = HashBits(38);

    /// Creates a `HashBits` from a raw bit count, if it is in `1..=63`.
    #[inline]
    pub const fn new(bits: u8) -> Option<Self> {
        match bits {
            1..=63 => Some(Self(bits)),
            _ => None,
        }
    }

    /// The raw bit count.
    #[inline]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl TryFrom<u8> for HashBits {
    type Error = RstError;

    fn try_from(bits: u8) -> Result<Self, Self::Error> {
        Self::new(bits).ok_or(RstError::InvalidHashBits { bits })
    }
}

impl From<HashBits> for u8 {
    fn from(bits: HashBits) -> Self {
        bits.0
    }
}

impl std::fmt::Display for HashBits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The xxHash variant used to hash RST keys.
///
/// Riot switched from xxHash64 to XXH3 (64-bit) in game patch 14.15.  Which one
/// a given file uses cannot be derived from the file itself — it is part of the
/// [`RstFormat`].  Note that the algorithm is only consulted when hashing a
/// *key string*; reading or iterating an existing table never needs it.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RstHashAlgo {
    /// xxHash64 (seed 0) — game patches before 14.15.
    Xxh64,
    /// XXH3, 64-bit (seed 0) — game patch 14.15 and later.
    Xxh3,
}

impl RstHashAlgo {
    /// Hashes the raw bytes of `key` (already lowercased) without truncation.
    #[inline]
    fn hash_raw(self, key: &[u8]) -> u64 {
        match self {
            RstHashAlgo::Xxh64 => xxh64::xxh64(key, 0),
            RstHashAlgo::Xxh3 => xxh3::xxh3_64(key),
        }
    }
}

/// A fully-resolved description of how an RST file packs and hashes its entries.
///
/// Three independent dimensions, none fully derivable from the file alone:
/// - `version` — structural framing (font config, trailing byte).
/// - `hash_bits` — the bit at which each `u64` entry splits into
///   `hash = v & ((1 << hash_bits) - 1)` and `offset = v >> hash_bits`.
///   40 for V2/V3, 39 or 38 for V4/V5.
/// - `hash_algo` — [`Xxh64`](RstHashAlgo::Xxh64) or [`Xxh3`](RstHashAlgo::Xxh3);
///   only used when hashing a key string.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RstFormat {
    /// Structural file version.
    pub version: RstVersion,
    /// Number of low bits of each entry that hold the hash (40 / 39 / 38).
    pub hash_bits: HashBits,
    /// Algorithm used to hash key strings.
    pub hash_algo: RstHashAlgo,
}

impl RstFormat {
    /// The current live-game format: V5, 38-bit, XXH3.
    pub const LATEST: RstFormat = RstFormat {
        version: RstVersion::V5,
        hash_bits: HashBits::B38,
        hash_algo: RstHashAlgo::Xxh3,
    };

    /// Builds a format explicitly.
    #[inline]
    pub const fn new(version: RstVersion, hash_bits: HashBits, hash_algo: RstHashAlgo) -> Self {
        Self {
            version,
            hash_bits,
            hash_algo,
        }
    }

    /// Derives the format from a `version` byte and a numeric game patch
    /// (e.g. `1502` for patch 15.2), matching the rules used by CDTB and
    /// ritobin:
    ///
    /// - algorithm: XXH3 for patch ≥ 14.15 (`1415`), otherwise xxHash64.
    /// - `hash_bits`: 40 for V2/V3; for V4/V5, 38 for patch ≥ 15.2 (`1502`),
    ///   otherwise 39.
    pub fn for_patch(version: RstVersion, patch: u32) -> Self {
        let hash_algo = if patch >= 1415 {
            RstHashAlgo::Xxh3
        } else {
            RstHashAlgo::Xxh64
        };
        let hash_bits = match version {
            RstVersion::V2 | RstVersion::V3 => HashBits::B40,
            RstVersion::V4 | RstVersion::V5 => {
                if patch >= 1502 {
                    HashBits::B38
                } else {
                    HashBits::B39
                }
            }
        };
        Self {
            version,
            hash_bits,
            hash_algo,
        }
    }

    /// Bitmask isolating the hash portion of a packed entry.
    #[inline]
    pub fn hash_mask(&self) -> u64 {
        (1u64 << self.hash_bits.get()) - 1
    }

    /// Hashes and masks `key` into an [`RstHash`] for this format.
    ///
    /// The key is ASCII-lowercased before hashing (keys are always ASCII in
    /// practice; non-ASCII bytes are hashed as-is), then truncated to
    /// `hash_bits`.
    #[inline]
    pub fn hash_of(&self, key: impl AsRef<str>) -> RstHash {
        let lowered = key.as_ref().to_ascii_lowercase();
        RstHash(self.hash_algo.hash_raw(lowered.as_bytes()) & self.hash_mask())
    }

    /// Packs a `hash` and a string `offset` into a single `u64` entry.
    ///
    /// The hash is masked to `hash_bits` first, so a hash carried over from a
    /// wider format (e.g. a 40-bit table being written narrower) can't bleed
    /// into the offset field.
    #[inline]
    pub fn pack(&self, hash: RstHash, offset: u64) -> u64 {
        (hash.0 & self.hash_mask()) | (offset << self.hash_bits.get())
    }

    /// Unpacks a raw `u64` entry into its `(hash, offset)` components.
    #[inline]
    pub fn unpack(&self, raw: u64) -> (RstHash, u64) {
        (RstHash(raw & self.hash_mask()), raw >> self.hash_bits.get())
    }
}
