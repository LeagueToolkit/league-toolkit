use crate::{
    property::{Kind, NoMeta},
    traits::{PropertyExt, ReadProperty as _, WriteProperty as _},
    Error,
};
use std::io;

use super::values::{self, *};

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
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(bound = "for <'dee> M: serde::Serialize + serde::Deserialize<'dee>")
        )]
        #[cfg_attr(feature = "serde", serde(tag = "kind", content = "value"))]
        #[derive(Clone, Debug, PartialEq)]
        /// The value of a property inside a [`crate::BinObject`]. Holds the type of the value, and the value itself.
        pub enum PropertyValueEnum<M = NoMeta> {
            $( $variant (self::$variant<M>), )*
        }


        impl<M: Default> PropertyValueEnum<M> {
            pub fn from_reader<R: io::Read + std::io::Seek + ?Sized>(
                reader: &mut R,
                kind: Kind,
                legacy: bool,
            ) -> Result<Self, Error> {
                Ok(match kind {
                    $(Kind::$variant => values::$variant::from_reader(reader, legacy)?.into()),*
                })
            }

        }
        impl<M: Clone> PropertyValueEnum<M> {
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
        impl<M> PropertyValueEnum<M> {
            #[inline(always)]
            #[must_use]
            pub fn kind(&self) -> Kind {
                match self {
                    $(Self::$variant(_) => Kind::$variant,)*
                }
            }

            #[inline(always)]
            #[must_use]
            pub fn no_meta(self) -> PropertyValueEnum<NoMeta> {
                 match self {
                     $(Self::$variant(i) => PropertyValueEnum::$variant(i.no_meta()),)*
                 }
            }

        }

        impl<M> PropertyExt for PropertyValueEnum<M> {
            type Meta = M;
            fn meta(&self) -> &Self::Meta {
                 match self {
                     $(Self::$variant(i) => i.meta(),)*
                 }
            }
            fn meta_mut(&mut self) -> &mut Self::Meta {
                 match self {
                     $(Self::$variant(i) => i.meta_mut(),)*
                 }
            }

            fn size(&self, include_header: bool) -> usize {
                 match self {
                     $(Self::$variant(i) => i.size(include_header),)*
                 }
            }
            fn size_no_header(&self) -> usize {
                 match self {
                     $(Self::$variant(i) => i.size_no_header(),)*
                 }
            }
        }

        $(
            impl<M> From<values::$variant<M>> for PropertyValueEnum<M> {
                fn from(other: values::$variant<M>) -> Self {
                    Self::$variant(other)
                }
            }
        )*
    };
}

variants!(create_enum);
