use crate::core::meta::traits::{PropertyValue, ReadProperty};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct StringValue(pub String);

impl PropertyValue for StringValue {
    fn size_no_header(&self) -> usize {
        self.0.len() + 2
    }
}

impl ReadProperty for StringValue {
    fn from_reader<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        use crate::util::ReaderExt as _;
        use byteorder::LE;
        Ok(Self(reader.read_len_prefixed_string::<LE>()?))
    }
}
