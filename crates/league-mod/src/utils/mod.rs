use eyre::eyre;
use regex::Regex;

pub fn is_valid_mod_name(name: impl AsRef<str>) -> bool {
    Regex::new(r"^[[:word:]-]+$")
        .unwrap()
        .is_match(name.as_ref())
}

pub fn validate_mod_name(name: impl AsRef<str>) -> eyre::Result<()> {
    if !is_valid_mod_name(name) {
        return Err(eyre!(
            "Invalid mod name, must be alphanumeric and contain no spaces or special characters"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_mod_name_valid() {
        assert!(is_valid_mod_name("test"));
        assert!(is_valid_mod_name("test-123"));
        assert!(!is_valid_mod_name("test 123"));
        assert!(!is_valid_mod_name("test!123"));
        assert!(!is_valid_mod_name("Nice mod: ([test])@"));
    }
}
