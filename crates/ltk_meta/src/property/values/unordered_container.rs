use crate::{
    property::Kind,
    stream::{layout::Numbering, owned},
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
        owned::read_from(
            reader,
            Kind::UnorderedContainer,
            Numbering::from_legacy(legacy),
            |cur| owned::read_container(cur).map(Self),
        )
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

impl<S: Into<Container>> From<S> for UnorderedContainer {
    fn from(value: S) -> Self {
        Self(value.into())
    }
}

impl AsRef<Container> for UnorderedContainer {
    fn as_ref(&self) -> &Container {
        &self.0
    }
}

impl std::ops::Deref for UnorderedContainer {
    type Target = Container;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
