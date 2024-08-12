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

use crate::core::meta::{property::BinPropertyKind, traits::ReadProperty as _, ParseError};

use enum_dispatch::enum_dispatch;

macro_rules! enum_construct {
    ($item:expr, $method:expr, [$($variant:ident),*]) => {
        match $item {
            $(BinPropertyKind::$variant => paste::paste! {
                Self::$variant([<$variant Value>]::$method)
            },)*
            _ => unimplemented!("{:?}", $item),
        }
    };
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "kind", content = "value"))]
#[derive(Clone, Debug, PartialEq)]
#[enum_dispatch(PropertyValue)]
pub enum PropertyValueEnum {
    None(NoneValue),
    Bool(BoolValue),
    I8(I8Value),
    U8(U8Value),
    I16(I16Value),
    U16(U16Value),
    I32(I32Value),
    U32(U32Value),
    I64(I64Value),
    U64(U64Value),
    F32(F32Value),
    Vector2(Vector2Value),
    Vector3(Vector3Value),
    Vector4(Vector4Value),
    Matrix44(Matrix44Value),
    Color(ColorValue),
    String(StringValue),
    Hash(HashValue),
    WadChunkLink(WadChunkLinkValue),
    Container(ContainerValue),
    UnorderedContainer(UnorderedContainerValue),
    Struct(StructValue),
    Embedded(EmbeddedValue),
    ObjectLink(ObjectLinkValue),
    Optional(OptionalValue),
    Map(MapValue),
    BitBool(BitBoolValue),
}

impl PropertyValueEnum {
    pub fn from_reader<R: io::Read>(
        reader: &mut R,
        kind: BinPropertyKind,
        legacy: bool,
    ) -> Result<Self, ParseError> {
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
}
