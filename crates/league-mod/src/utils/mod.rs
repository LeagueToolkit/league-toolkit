use eyre::eyre;
use regex::Regex;

pub fn validate_mod_name(name: impl AsRef<str>) -> eyre::Result<()> {
    let check = Regex::new(r"^[[:word:]-]+$").unwrap();
    if !check.is_match(name.as_ref()) {
        return Err(eyre!(
            "Invalid mod name, must be alphanumeric and contain no spaces or special characters"
        ));
    }

    Ok(())
}
