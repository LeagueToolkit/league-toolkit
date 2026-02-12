use crate::{
    property::Kind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};
use byteorder::LE;
use ltk_io_ext::{ReaderExt, WriterExt};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct String(pub std::string::String);

impl PropertyValueExt for String {
    const KIND: Kind = Kind::String;
}

impl PropertyExt for String {
    fn size_no_header(&self) -> usize {
        self.0.len() + 2
    }
}

impl ReadProperty for String {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        _legacy: bool,
    ) -> Result<Self, crate::Error> {
        Ok(Self(reader.read_sized_string_u16::<LE>()?))
    }
}

impl WriteProperty for String {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        writer.write_len_prefixed_string::<LE, _>(&self.0)
    }
}
