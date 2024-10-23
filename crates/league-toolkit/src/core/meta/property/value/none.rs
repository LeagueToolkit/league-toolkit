use crate::core::meta::traits::{PropertyValue, ReadProperty, WriteProperty};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NoneValue;

impl PropertyValue for NoneValue {
    fn size_no_header(&self) -> usize {
        0
    }
}

impl ReadProperty for NoneValue {
    fn from_reader<R: std::io::Read + std::io::Seek>(
        _reader: &mut R,
        _legacy: bool,
    ) -> Result<Self, crate::core::meta::ParseError> {
        Ok(Self)
    }
}
impl WriteProperty for NoneValue {
    fn to_writer<R: std::io::Write + ?Sized>(
        &self,
        writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
}
