use crate::traits::{PropertyValue, ReadProperty, WriteProperty};
use byteorder::LE;
use ltk_io_ext::{ReaderExt, WriterExt};

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
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        _legacy: bool,
    ) -> Result<Self, crate::Error> {
        Ok(Self(reader.read_len_prefixed_string::<LE>()?))
    }
}

impl WriteProperty for StringValue {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        writer.write_len_prefixed_string::<LE, _>(&self.0)
    }
}
