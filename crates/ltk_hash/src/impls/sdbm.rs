//! Provides the SDBM hash that the client computes for the keys of an inibin file.
//!
//! The multiplier is 65599. The client converts ASCII uppercase letters to lowercase and adds
//! each byte as a signed 8-bit integer.

/// The multiplier of the SDBM hash.
///
/// The hash is linear in its state: the hash of `input` from the state `s` is
/// `s * MULTIPLIER^len + hash_lower(input)`.
pub const MULTIPLIER: u32 = 65599;

/// Returns the SDBM hash of `byte`, starting from the hash state `hash`. Converts an ASCII
/// uppercase letter to lowercase before hashing.
pub const fn hash_lower_byte(hash: u32, byte: u8) -> u32 {
    hash.wrapping_mul(MULTIPLIER)
        .wrapping_add(byte.to_ascii_lowercase() as i8 as u32)
}

/// Returns the SDBM hash of the bytes of `input`. Converts ASCII uppercase letters to lowercase
/// before hashing.
pub const fn hash_lower(input: &str) -> u32 {
    hash_lower_from(0, input)
}

/// Returns the SDBM hash of the bytes of `input`, starting from the hash state `hash`. Converts
/// ASCII uppercase letters to lowercase before hashing.
///
/// `hash_lower_from(hash_lower(a), b)` is equal to `hash_lower` of `a` concatenated with `b`.
pub const fn hash_lower_from(hash: u32, input: &str) -> u32 {
    let bytes = input.as_bytes();
    let mut hash = hash;
    let mut i = 0;
    while i < bytes.len() {
        hash = hash_lower_byte(hash, bytes[i]);
        i += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_lower_matches_reference_value() {
        assert_eq!(hash_lower("test"), 0x4745d132);
    }

    #[test]
    fn hash_lower_ignores_ascii_case() {
        assert_eq!(hash_lower("TEST"), hash_lower("test"));
    }

    #[test]
    fn hash_lower_returns_zero_for_empty_input() {
        assert_eq!(hash_lower(""), 0);
    }

    #[test]
    fn hash_lower_sign_extends_non_ascii_bytes() {
        assert_eq!(hash_lower("\u{e9}"), 0xffc2f0a6);
    }

    #[test]
    fn hash_lower_from_continues_hash_state() {
        let hash = hash_lower_from(hash_lower("System"), "*GroupPart1");
        assert_eq!(hash, hash_lower("System*GroupPart1"));
        assert_eq!(hash, 0x0616933a);
    }
}
