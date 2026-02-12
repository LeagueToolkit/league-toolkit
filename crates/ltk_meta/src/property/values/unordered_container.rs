use crate::{
    property::Kind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};

use super::Container;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug, Default)]
pub struct UnorderedContainer(pub Container);

impl PropertyValueExt for UnorderedContainer {
    const KIND: Kind = Kind::UnorderedContainer;
}

impl PropertyExt for UnorderedContainer {
    fn size_no_header(&self) -> usize {
        self.0.size_no_header()
    }
}

impl ReadProperty for UnorderedContainer {
    fn from_reader<R: std::io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, crate::Error> {
        Ok(Self(Container::from_reader(reader, legacy)?))
    }
}

impl WriteProperty for UnorderedContainer {
    fn to_writer<R: std::io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut R,
        legacy: bool,
    ) -> Result<(), std::io::Error> {
        self.0.to_writer(writer, legacy)
    }
}
