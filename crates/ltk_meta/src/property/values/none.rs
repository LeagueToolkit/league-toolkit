use crate::{
    property::Kind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub struct None;
impl PropertyValueExt for None {
    const KIND: Kind = Kind::None;
}
impl PropertyExt for None {
    fn size_no_header(&self) -> usize {
        0
    }
}

impl ReadProperty for None {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        _reader: &mut R,
        _legacy: bool,
    ) -> Result<Self, crate::Error> {
        Ok(Self)
    }
}
impl WriteProperty for None {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        _writer: &mut R,
        _legacy: bool,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
}
