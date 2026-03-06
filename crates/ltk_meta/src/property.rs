pub mod values;

mod kind;
pub use kind::*;

mod r#enum;
pub use r#enum::*;

use super::traits::{ReaderExt as _, WriterExt as _};
use super::Error;
use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use std::io;

use crate::{traits::PropertyExt, Bin};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoMeta;

#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
)]
#[derive(Clone, PartialEq, Debug)]
pub struct BinProperty<M = NoMeta> {
    pub name_hash: u32,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub value: PropertyValueEnum<M>,
}

impl<M> BinProperty<M> {
    #[inline(always)]
    #[must_use]
    pub fn no_meta(self) -> BinProperty<NoMeta> {
        BinProperty {
            name_hash: self.name_hash,
            value: self.value.no_meta(),
        }
    }

    /// Read a BinProperty from a reader. This will read the name_hash, prop kind and then value, in that order.
    pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        legacy: bool,
    ) -> Result<Self, Error>
    where
        M: Default,
    {
        let name_hash = reader.read_u32::<LE>()?;
        let kind = reader.read_property_kind(legacy)?;

        Ok(Self {
            name_hash,
            value: PropertyValueEnum::from_reader(reader, kind, legacy)?,
        })
    }
    pub fn to_writer<W: io::Write + std::io::Seek + ?Sized>(
        &self,
        writer: &mut W,
    ) -> Result<(), io::Error>
    where
        M: Clone,
    {
        writer.write_u32::<LE>(self.name_hash)?;
        writer.write_property_kind(self.value.kind())?;

        self.value.to_writer(writer)?;
        Ok(())
    }
    pub fn size(&self) -> usize {
        5 + self.value.size_no_header()
    }
}
