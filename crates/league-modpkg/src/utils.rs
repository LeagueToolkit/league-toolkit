pub fn non_empty_string(value: String) -> Option<String> {
    match value.is_empty() {
        true => None,
        false => Some(value),
    }
}

pub fn length_prefixed_string_size(value: impl AsRef<str>) -> usize {
    value.as_ref().len() + 4
}
