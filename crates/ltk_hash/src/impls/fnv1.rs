//! The FNV-1 32-bit hash of Wwise names.
//!
//! Wwise hashes event, switch, state, game parameter and bank names with FNV-1. The bin names of
//! [`BinHash`](crate::BinHash) use FNV-1a. The two functions differ in the order of the multiply
//! and the XOR.

/// The FNV-1 32-bit offset basis.
const OFFSET_BASIS: u32 = 0x811c9dc5;

/// The FNV-1 32-bit prime.
const PRIME: u32 = 0x01000193;

/// The length limit of a bank name, without the NUL: `AK_MAX_PATH` (260) minus 1.
const MAX_BANK_NAME_LEN: usize = 259;

/// Returns the FNV-1 32-bit hash of `input` with `A` to `Z` lowercased.
///
/// The hash is `AK::SoundEngine::GetIDFromString`. It lowercases ASCII only. Every byte outside
/// `A` to `Z` is hashed as it is.
///
/// ```
/// use ltk_hash::fnv1;
///
/// assert_eq!(fnv1::hash_lower("Play_vo_Ahri_Attack"), 0x2890d955);
/// assert_eq!(fnv1::hash_lower("PLAY_VO_AHRI_ATTACK"), 0x2890d955);
/// ```
pub const fn hash_lower(input: &str) -> u32 {
    hash_lower_bytes(input.as_bytes())
}

/// Returns the bank ID of the bank file name `name`.
///
/// Truncates `name` to 259 bytes, cuts it at its last `.`, and hashes the rest with
/// [`hash_lower`]. A name without a `.` is hashed whole. The function does not remove a
/// directory. The function is `AK::SoundEngine::GetBankIDFromString`.
///
/// ```
/// use ltk_hash::fnv1;
///
/// assert_eq!(fnv1::bank_id("sett_base_sfx_audio.bnk"), 0xe9b70b40);
/// assert_eq!(fnv1::bank_id("sett_base_sfx_audio"), 0xe9b70b40);
/// ```
pub fn bank_id(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let bytes = &bytes[..bytes.len().min(MAX_BANK_NAME_LEN)];
    let stem = match bytes.iter().rposition(|&b| b == b'.') {
        Some(dot) => &bytes[..dot],
        None => bytes,
    };
    hash_lower_bytes(stem)
}

const fn hash_lower_bytes(bytes: &[u8]) -> u32 {
    let mut hash = OFFSET_BASIS;
    let mut i = 0;
    while i < bytes.len() {
        hash = hash.wrapping_mul(PRIME);
        hash ^= bytes[i].to_ascii_lowercase() as u32;
        i += 1;
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bank_ids_match_the_shipped_bank_headers() {
        assert_eq!(bank_id("sett_base_sfx_audio"), 0xe9b70b40);
        assert_eq!(bank_id("zed_skin11_sfx_audio.bnk"), 0xdf17207f);
        assert_eq!(bank_id("aatrox_skin07_sfx_events.bnk"), 0x962e8665);
    }

    #[test]
    fn hash_multiplies_before_it_xors() {
        assert_eq!(hash_lower("sett_base_sfx_audio"), 0xe9b70b40);
        assert_eq!(
            crate::impls::fnv1a::hash_lower("sett_base_sfx_audio"),
            0x9cd9f5de
        );
    }

    #[test]
    fn hash_lowercases_ascii_only() {
        assert_eq!(hash_lower("ABC"), hash_lower("abc"));
        assert_ne!(hash_lower("\u{c9}"), hash_lower("\u{e9}"));
    }

    #[test]
    fn bank_id_cuts_at_the_last_dot() {
        assert_eq!(bank_id("a.b.bnk"), hash_lower("a.b"));
        assert_eq!(bank_id("no_extension"), hash_lower("no_extension"));
        assert_eq!(bank_id(".bnk"), hash_lower(""));
    }

    #[test]
    fn bank_id_truncates_to_259_bytes_before_it_cuts() {
        let long = format!("{}.bnk", "a".repeat(258));
        assert_eq!(bank_id(&long), hash_lower(&"a".repeat(258)));
        let longer = format!("{}.bnk", "a".repeat(259));
        assert_eq!(bank_id(&longer), hash_lower(&"a".repeat(259)));
    }
}
