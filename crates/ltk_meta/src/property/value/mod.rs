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
    property::BinPropertyKind, traits::ReadProperty as _, traits::WriteProperty as _, Error,
};

use enum_dispatch::enum_dispatch;

macro_rules! enum_construct {
    ($item:expr, $method:expr, [$($variant:ident),*]) => {
        match $item {
            $(BinPropertyKind::$variant => paste::paste! {
                Self::$variant([<$variant Value>]::$method)
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
#[enum_dispatch(PropertyValue)]
/// The value part of a [`super::BinProperty`]. Holds the type of the value, and the value itself.
pub enum PropertyValueEnum {
    None(pub NoneValue),
    Bool(pub BoolValue),
    I8(pub I8Value),
    U8(pub U8Value),
    I16(pub I16Value),
    U16(pub U16Value),
    I32(pub I32Value),
    U32(pub U32Value),
    I64(pub I64Value),
    U64(pub U64Value),
    F32(pub F32Value),
    Vector2(pub Vector2Value),
    Vector3(pub Vector3Value),
    Vector4(pub Vector4Value),
    Matrix44(pub Matrix44Value),
    Color(pub ColorValue),
    String(pub StringValue),
    Hash(pub HashValue),
    WadChunkLink(pub WadChunkLinkValue),
    Container(pub ContainerValue),
    UnorderedContainer(pub UnorderedContainerValue),
    Struct(pub StructValue),
    Embedded(pub EmbeddedValue),
    ObjectLink(pub ObjectLinkValue),
    Optional(pub OptionalValue),
    Map(pub MapValue),
    BitBool(pub BitBoolValue),
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
    #[must_use]
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
