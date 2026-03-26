/// Compute SDBM hash of a lowercased string.
pub fn hash_lower(input: &str) -> u32 {
    let mut hash: u32 = 0;
    for c in input.chars().flat_map(|c| c.to_lowercase()) {
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

/// Compute SDBM hash of two strings joined by a delimiter, all lowercased.
///
/// Used for inibin keys: `hash_lower_with_delimiter(section, property, '*')`
pub fn hash_lower_with_delimiter(a: &str, b: &str, delimiter: char) -> u32 {
    let mut hash: u32 = 0;

    let chars = a
        .chars()
        .chain(std::iter::once(delimiter))
        .chain(b.chars())
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
}
