use xxhash_rust::xxh64::xxh64;

/// Computes the XXHash64 of `input` bytes with the given `seed`.
///
/// This is a thin wrapper around [`xxhash_rust::xxh64::xxh64`] that is
/// re-exported so downstream crates can depend on a single hashing crate.
#[inline]
pub fn xxhash64(input: &[u8], seed: u64) -> u64 {
    xxh64(input, seed)
}
