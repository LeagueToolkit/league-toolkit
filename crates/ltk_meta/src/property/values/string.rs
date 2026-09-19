use crate::{
    property::Kind,
    stream::{layout::Numbering, owned},
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};
use byteorder::LE;
use ltk_io_ext::WriterExt;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct String {
    pub value: std::string::String,
}

impl String {
    #[inline(always)]
    #[must_use]
    pub fn new(value: std::string::String) -> Self {
        Self { value }
    }
}

impl PropertyValueExt for String {
    const KIND: Kind = Kind::String;
}

impl PropertyExt for String {
    fn size_no_header(&self) -> usize {
        self.value.len() + 2
    }
}

impl ReadProperty for String {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error> {
        owned::read_from(
            reader,
            Kind::String,
            Numbering::from_legacy(legacy),
            owned::read_string,
        )
    }
}

impl WriteProperty for String {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        writer.write_len_prefixed_string::<LE, _>(&self.value)
    }
}

impl<S: Into<std::string::String>> From<S> for String {
    fn from(value: S) -> Self {
        Self::new(value.into())
    }
}
impl AsRef<std::string::String> for String {
    fn as_ref(&self) -> &std::string::String {
        &self.value
    }
}
impl AsRef<str> for String {
    fn as_ref(&self) -> &str {
        self.value.as_str()
    }
}

impl std::ops::Deref for String {
    type Target = std::string::String;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
