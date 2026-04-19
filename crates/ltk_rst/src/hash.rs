use ltk_hash::xxhash::xxhash64;

use crate::version::RstHashType;

/// Computes the masked XXHash64 of `key` lowercased as UTF-8, suitable for use
/// as an RST entry hash (without the string-offset component).
///
/// The result is masked to the bit-width defined by `hash_type`:
/// - [`RstHashType::Complex`] → lower 40 bits
/// - [`RstHashType::Simple`]  → lower 39 bits
pub fn compute_hash(key: &str, hash_type: RstHashType) -> u64 {
    let lowered = key.to_lowercase();
    let raw = xxhash64(lowered.as_bytes(), 0);
    raw & hash_type.hash_mask()
}

/// Packs a pre-computed masked `hash` together with a string `offset` into the
/// single `u64` value written into the RST hash table.
#[inline]
pub fn pack_entry(hash: u64, offset: u64, hash_type: RstHashType) -> u64 {
    hash | (offset << hash_type.offset_shift())
}

/// Unpacks a raw RST hash-table entry into `(hash, offset)`.
#[inline]
pub fn unpack_entry(entry: u64, hash_type: RstHashType) -> (u64, u64) {
    let hash = entry & hash_type.hash_mask();
    let offset = entry >> hash_type.offset_shift();
    (hash, offset)
}
