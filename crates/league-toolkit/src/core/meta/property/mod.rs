use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::io;
use value::PropertyValueEnum;

use super::ParseError;

pub mod value;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, IntoPrimitive, TryFromPrimitive,
)]
#[repr(u8)]
pub enum BinPropertyKind {
    // PRIMITIVE TYPES
    None = 0,
    Bool = 1,
    I8 = 2,
    U8 = 3,
    I16 = 4,
    U16 = 5,
    I32 = 6,
    U32 = 7,
    I64 = 8,
    U64 = 9,
    F32 = 10,
    Vector2 = 11,
    Vector3 = 12,
    Vector4 = 13,
    Matrix44 = 14,
    Color = 15,
    String = 16,
    Hash = 17,
    WadChunkLink = 18, // newly added

    // COMPLEX TYPES
    Container = 128,
    UnorderedContainer = 128 | 1,
    Struct = 128 | 2,
    Embedded = 128 | 3,
    ObjectLink = 128 | 4,
    Optional = 128 | 5,
    Map = 128 | 6,
    BitBool = 128 | 7,
}

impl BinPropertyKind {
    /// The WadChunkLink bin property type was newly added by Riot. For some reason they decided to put it in the middle of the enum,
    /// so we need to handle cases from before and after it existed.
    ///
    /// "Legacy" property types need to be fudged around to pretend like WadChunkLink always existed, from our pov.
    ///
    /// "Non-legacy" property types can just be used as is.
    ///
    pub fn unpack(raw: u8, legacy: bool) -> Result<BinPropertyKind, ParseError> {
        use BinPropertyKind as BPK;
        if !legacy {
            // TODO (alan): don't panic here
            return Ok(BPK::try_from_primitive(raw)?);
        }
        let mut fudged = raw;

        // if the prop type comes after where WadChunkLink is now, we need to
        if fudged >= BPK::WadChunkLink.into() && fudged < BPK::Container.into() {
            fudged -= Into::<u8>::into(BPK::WadChunkLink);
            fudged |= Into::<u8>::into(BPK::Container);
        }

        if fudged >= BPK::UnorderedContainer.into() {
            fudged += 1;
        }

        Ok(BinPropertyKind::try_from_primitive(fudged)?)
    }

    pub fn read<R: io::Read>(
        self,
        reader: &mut R,
        legacy: bool,
    ) -> Result<PropertyValueEnum, super::ParseError> {
        PropertyValueEnum::from_reader(reader, self, legacy)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, PartialEq, Debug)]
pub struct BinProperty {
    pub name_hash: u32,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub value: PropertyValueEnum,
}

impl BinProperty {
    pub fn from_reader<R: io::Read>(reader: &mut R, legacy: bool) -> Result<Self, ParseError> {
        use super::traits::ReaderExt;
        use byteorder::{ReadBytesExt as _, LE};
        let name_hash = reader.read_u32::<LE>()?;
        let kind = reader.read_property_kind(legacy)?;

        Ok(Self {
            name_hash,
            value: PropertyValueEnum::from_reader(reader, kind, legacy)?,
        })
    }
    pub fn size(&self) -> usize {
        todo!()
    }
}
