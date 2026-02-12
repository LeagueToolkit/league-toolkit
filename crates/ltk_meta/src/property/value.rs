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
    property::Kind,
    traits::{ReadProperty as _, WriteProperty as _},
    Error,
};

use enum_dispatch::enum_dispatch;

macro_rules! variants {
    ($macro:ident $(, $args:tt)* ) => {
        $macro! {
            $( $args )*
                [
                    None,
                    Bool,
                    I8, U8,
                    I16, U16,
                    I32, U32,
                    I64, U64,
                    F32,
                    Vector2, Vector3, Vector4,
                    Matrix44,
                    Color,
                    String,
                    Hash,
                    WadChunkLink,
                    Struct,
                    Embedded,
                    ObjectLink,
                    BitBool,

                    Container, UnorderedContainer,
                    Optional,
                    Map,
                ]

        }
    };
}

macro_rules! create_enum {
    ([$( $variant:ident, )*]) => {
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(feature = "serde", serde(tag = "kind", content = "value"))]
        #[derive(Clone, Debug, PartialEq)]
        #[enum_dispatch(PropertyExt)]
        /// The value part of a [`super::BinProperty`]. Holds the type of the value, and the value itself.
        pub enum PropertyValueEnum {
            $( $variant (pub self::$variant), )*
        }


        impl PropertyValueEnum {
            #[must_use]
            pub fn kind(&self) -> Kind {
                match self {
                    $(Self::$variant(_) => Kind::$variant,)*
                }
            }


            pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
                reader: &mut R,
                kind: Kind,
                legacy: bool,
            ) -> Result<Self, Error> {
                Ok(match kind {
                    $(Kind::$variant => $variant::from_reader(reader, legacy)?.into()),*
                })
            }


            pub fn to_writer<W: io::Write + io::Seek + ?Sized>(
                &self,
                writer: &mut W,
            ) -> Result<(), io::Error> {
                match self {
                    $(Self::$variant(inner) => inner.to_writer(writer, false)?,)*
                };
                Ok(())
            }
        }
    };
}

variants!(create_enum);
