pub fn optional_string_len<'a>(string: impl Into<Option<&'a String>>) -> usize {
    string.into().as_ref().map(|n| n.len()).unwrap_or_default()
}

pub fn optional_string_write(s: &Option<String>) -> Option<Vec<u8>> {
    s.as_ref().map(|s| s.as_bytes().to_vec())
}
pub fn optional_string_read(s: Vec<u8>) -> Result<Option<String>, std::string::FromUtf8Error> {
    String::from_utf8(s).map(Some)
}
