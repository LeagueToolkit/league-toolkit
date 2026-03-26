/// Compute SDBM hash of a lowercased string.
pub fn hash_lower(input: impl AsRef<str>) -> u32 {
    let mut hash: u32 = 0;
    for c in input.as_ref().chars().flat_map(|c| c.to_lowercase()) {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        for &byte in encoded.as_bytes() {
            hash = (byte as u32)
                .wrapping_add(hash.wrapping_shl(6))
                .wrapping_add(hash.wrapping_shl(16))
                .wrapping_sub(hash);
        }
    }
    hash
}

/// Compute SDBM hash of an inibin `section*property` key pair (lowercased, `*` delimiter).
///
/// Convenience wrapper around [`hash_lower_with_delimiter`] with the standard inibin delimiter.
///
/// ```
/// # use ltk_hash::sdbm;
/// let key = sdbm::hash_inibin_key("DATA", "AttackRange");
/// assert_eq!(key, sdbm::hash_lower_with_delimiter("DATA", "AttackRange", '*'));
/// ```
pub fn hash_inibin_key(section: impl AsRef<str>, property: impl AsRef<str>) -> u32 {
    hash_lower_with_delimiter(section, property, '*')
}

/// Compute SDBM hash of two strings joined by a delimiter, all lowercased.
///
/// For inibin keys, prefer [`hash_inibin_key`] which defaults the `*` delimiter.
pub fn hash_lower_with_delimiter(a: impl AsRef<str>, b: impl AsRef<str>, delimiter: char) -> u32 {
    let mut hash: u32 = 0;

    let chars = a
        .as_ref()
        .chars()
        .chain(std::iter::once(delimiter))
        .chain(b.as_ref().chars())
        .flat_map(|c| c.to_lowercase());

    for c in chars {
        let mut buf = [0u8; 4];
        let encoded = c.encode_utf8(&mut buf);
        for &byte in encoded.as_bytes() {
            hash = (byte as u32)
                .wrapping_add(hash.wrapping_shl(6))
                .wrapping_add(hash.wrapping_shl(16))
                .wrapping_sub(hash);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_lower_basic() {
        // SDBM hash of "test" (all lowercase)
        let h = hash_lower("test");
        // Verify case-insensitivity
        assert_eq!(h, hash_lower("TEST"));
        assert_eq!(h, hash_lower("TeSt"));
    }

    #[test]
    fn test_hash_lower_empty() {
        assert_eq!(hash_lower(""), 0);
    }

    #[test]
    fn test_hash_lower_with_delimiter() {
        let h1 = hash_lower_with_delimiter("DATA", "AttackRange", '*');
        let h2 = hash_lower("data*attackrange");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_lower_with_delimiter_case_insensitive() {
        let h1 = hash_lower_with_delimiter("DATA", "AttackRange", '*');
        let h2 = hash_lower_with_delimiter("data", "attackrange", '*');
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_inibin_key() {
        let h1 = hash_inibin_key("DATA", "AttackRange");
        let h2 = hash_lower_with_delimiter("DATA", "AttackRange", '*');
        assert_eq!(h1, h2);
        // Case insensitive
        assert_eq!(h1, hash_inibin_key("data", "attackrange"));
    }

    #[test]
    fn test_hash_lower_accepts_string() {
        let s = String::from("test");
        assert_eq!(hash_lower(&s), hash_lower("test"));
        assert_eq!(hash_lower(s), hash_lower("test"));
    }

    #[test]
    fn test_hash_lower_with_delimiter_accepts_string() {
        let a = String::from("DATA");
        let b = String::from("AttackRange");
        assert_eq!(
            hash_lower_with_delimiter(&a, &b, '*'),
            hash_lower_with_delimiter("DATA", "AttackRange", '*')
        );
        assert_eq!(
            hash_lower_with_delimiter(a, b, '*'),
            hash_lower_with_delimiter("DATA", "AttackRange", '*')
        );
    }
}
