use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader},
};

use eyre::eyre;
use regex::Regex;

pub mod modpkg;

pub fn is_valid_slug(name: impl AsRef<str>) -> bool {
    Regex::new(r"^[[:word:]-]+$")
        .unwrap()
        .is_match(name.as_ref())
}

pub fn validate_mod_name(name: impl AsRef<str>) -> eyre::Result<()> {
    if !is_valid_slug(name) {
        return Err(eyre!(
            "Invalid mod name, must be alphanumeric and contain no spaces or special characters (You can set a display name later)"
        ));
    }

    Ok(())
}

pub fn validate_version_format(version: impl AsRef<str>) -> eyre::Result<()> {
    if !semver::Version::parse(version.as_ref()).is_ok() {
        return Err(eyre!(
            "Invalid version format, must be a valid semantic version"
        ));
    }

    Ok(())
}

pub fn load_wad_hashtable<R: io::Read>(reader: R) -> eyre::Result<HashMap<u64, String>> {
    let reader = BufReader::new(reader);
    let mut hashtable = HashMap::new();

    for line in reader.lines() {
        let line = line?;

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let hash = parts[0].trim();
            let path = parts[1].trim();
            hashtable.insert(u64::from_str_radix(hash, 16)?, path.to_string());
        }
    }

    Ok(hashtable)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_slug_valid() {
        assert!(is_valid_slug("test"));
        assert!(is_valid_slug("test-123"));
        assert!(!is_valid_slug("test 123"));
        assert!(!is_valid_slug("test!123"));
        assert!(!is_valid_slug("Nice mod: ([test])@"));
    }
}
