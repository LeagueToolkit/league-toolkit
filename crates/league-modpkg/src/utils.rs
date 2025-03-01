use binrw::NullString;

pub fn optional_string_len<'a>(string: impl Into<Option<&'a String>>) -> usize {
    string.into().as_ref().map(|n| n.len()).unwrap_or_default()
}

pub fn optional_string_write(s: &Option<String>) -> Option<Vec<u8>> {
    s.as_ref().map(|s| s.as_bytes().to_vec())
}
pub fn optional_string_read(s: Vec<u8>) -> Result<Option<String>, std::string::FromUtf8Error> {
    String::from_utf8(s).map(Some)
}

pub fn nullstr_read(s: NullString) -> Result<String, std::string::FromUtf8Error> {
    String::from_utf8(s.into())
}

pub fn nullstr_write<'a>(s: impl Into<&'a String>) -> NullString {
    NullString::from(s.into().as_str())
}

#[cfg(test)]
pub mod test {
    use binrw::{meta, BinWrite};
    use std::io::Cursor;

    pub fn written_size<I>(item: &I, expected_size: usize)
    where
        for<'a> I: BinWrite<Args<'a> = ()> + meta::WriteEndian,
    {
        let mut buf = Cursor::new(Vec::with_capacity(expected_size + 64));
        item.write(&mut buf).unwrap();

        assert_eq!(
            expected_size,
            buf.into_inner().len(),
            "comparing reported size with real size"
        );
    }
}
