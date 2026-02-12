//! Value types for [`super::BinProperty`].
mod container;
mod embedded;
mod map;
mod none;
mod optional;
mod primitives;
mod string;
mod r#struct;
mod unordered_container;

pub use container::*;
pub use embedded::*;
pub use map::*;
pub use none::*;
pub use optional::*;
pub use primitives::*;
pub use r#struct::*;
pub use string::*;
pub use unordered_container::*;

use std::io;

use crate::{
    property::BinPropertyKind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty as _, WriteProperty as _},
    Error,
};

use enum_dispatch::enum_dispatch;

macro_rules! enum_construct {
    ($item:expr, $method:expr, [$($variant:ident),*]) => {
        match $item {
            $(BinPropertyKind::$variant => paste::paste! {
                Self::$variant($variant::$method)
            },)*
        }
    };
}
macro_rules! enum_to_writer {
    ($item:expr, $writer:expr, [$($variant:ident),*]) => {
        match $item {
            $(Self::$variant(inner) => inner.to_writer($writer, false),)*
        }
    };
}
macro_rules! enum_kind {
    ($item:expr, [$($variant:ident),*]) => {
        match $item {
            $(Self::$variant(_) => BinPropertyKind::$variant,)*
        }
    };
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "value"))]
#[derive(Clone, Debug, PartialEq)]
#[enum_dispatch(PropertyExt)]
/// The value part of a [`super::BinProperty`]. Holds the type of the value, and the value itself.
pub enum PropertyValueEnum {
    None(pub self::None),
    Bool(pub self::Bool),
    I8(pub self::I8),
    U8(pub self::U8),
    I16(pub self::I16),
    U16(pub self::U16),
    I32(pub self::I32),
    U32(pub self::U32),
    I64(pub self::I64),
    U64(pub self::U64),
    F32(pub self::F32),
    Vector2(pub self::Vector2),
    Vector3(pub self::Vector3),
    Vector4(pub self::Vector4),
    Matrix44(pub self::Matrix44),
    Color(pub self::Color),
    String(pub self::String),
    Hash(pub self::Hash),
    WadChunkLink(pub self::WadChunkLink),
    Container(pub self::Container),
    UnorderedContainer(pub self::UnorderedContainer),
    Struct(pub self::Struct),
    Embedded(pub self::Embedded),
    ObjectLink(pub self::ObjectLink),
    Optional(pub self::Optional),
    Map(pub self::Map),
    BitBool(pub self::BitBool),
}

impl PropertyValueEnum {
    #[must_use]
    pub fn kind(&self) -> BinPropertyKind {
        enum_kind!(
            self,
            [
                None,
                Bool,
                I8,
                U8,
                I16,
                U16,
                I32,
                U32,
                I64,
                U64,
                F32,
                Vector2,
                Vector3,
                Vector4,
                Matrix44,
                Color,
                String,
                Hash,
                WadChunkLink,
                Container,
                UnorderedContainer,
                Struct,
                Embedded,
                ObjectLink,
                Optional,
                Map,
                BitBool
            ]
        )
    }
    pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
        reader: &mut R,
        kind: BinPropertyKind,
        legacy: bool,
    ) -> Result<Self, Error> {
        Ok(enum_construct!(
            kind,
            from_reader(reader, legacy)?,
            [
                None,
                Bool,
                I8,
                U8,
                I16,
                U16,
                I32,
                U32,
                I64,
                U64,
                F32,
                Vector2,
                Vector3,
                Vector4,
                Matrix44,
                Color,
                String,
                Hash,
                WadChunkLink,
                Container,
                UnorderedContainer,
                Struct,
                Embedded,
                ObjectLink,
                Optional,
                Map,
                BitBool
            ]
        ))
    }

    pub fn to_writer<W: io::Write + io::Seek + ?Sized>(
        &self,
        writer: &mut W,
    ) -> Result<(), io::Error> {
        enum_to_writer!(
            self,
            writer,
            [
                None,
                Bool,
                I8,
                U8,
                I16,
                U16,
                I32,
                U32,
                I64,
                U64,
                F32,
                Vector2,
                Vector3,
                Vector4,
                Matrix44,
                Color,
                String,
                Hash,
                WadChunkLink,
                Container,
                UnorderedContainer,
                Struct,
                Embedded,
                ObjectLink,
                Optional,
                Map,
                BitBool
            ]
        )
    }
}
