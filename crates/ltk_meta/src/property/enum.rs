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

        $(
            impl<M> FromValue<M> for values::$variant<M> {
                fn from_value(value: &PropertyValueEnum<M>) -> Option<&Self> {
                    match value {
                        PropertyValueEnum::$variant(inner) => Some(inner),
                        _ => None,
                    }
                }

                fn from_value_mut(value: &mut PropertyValueEnum<M>) -> Option<&mut Self> {
                    match value {
                        PropertyValueEnum::$variant(inner) => Some(inner),
                        _ => None,
                    }
                }
            }
        )*

        /// A mutable borrow of the value inside a [`PropertyValueEnum`], one variant per [`Kind`].
        ///
        /// Unlike a `&mut PropertyValueEnum`, this cannot change which kind the value is: every
        /// variant borrows a concrete value type, so only the contents can be edited. That is what
        /// makes it safe to hand out for a value whose kind something else has already declared -
        /// see [`ValueSlot`](crate::ValueSlot).
        #[derive(Debug, PartialEq)]
        pub enum ValueMut<'a, M = NoMeta> {
            $( $variant (&'a mut self::$variant<M>), )*
        }

        impl<M> ValueMut<'_, M> {
            /// The kind of the borrowed value.
            #[inline(always)]
            #[must_use]
            pub fn kind(&self) -> Kind {
                match self {
                    $(Self::$variant(_) => Kind::$variant,)*
                }
            }
        }

        impl<M> PropertyValueEnum<M> {
            /// A mutable borrow of the value that cannot change its kind.
            ///
            /// # Examples
            ///
            /// ```
            /// use ltk_meta::{property::{values, ValueMut}, PropertyValueEnum};
            ///
            /// let mut value: PropertyValueEnum = values::I32::new(41).into();
            /// if let ValueMut::I32(i) = value.as_mut() {
            ///     i.value += 1;
            /// }
            /// assert_eq!(value, values::I32::new(42).into());
            /// ```
            #[inline(always)]
            pub fn as_mut(&mut self) -> ValueMut<'_, M> {
                match self {
                    $(Self::$variant(inner) => ValueMut::$variant(inner),)*
                }
            }
        }
    };
}

variants!(create_enum);

/// A value type that can be borrowed out of a [`PropertyValueEnum`].
///
/// Implemented for every type in [`values`]. It exists so [`PropertyValueEnum::get`] and
/// [`ValueSlot::get_mut`](crate::ValueSlot::get_mut) can reach one concrete value type without a
/// `match`; to handle every kind at once, match on [`PropertyValueEnum::as_mut`] instead.
pub trait FromValue<M>: Sized {
    /// The value as `Self`, if that is its kind.
    fn from_value(value: &PropertyValueEnum<M>) -> Option<&Self>;
    /// See [`FromValue::from_value`].
    fn from_value_mut(value: &mut PropertyValueEnum<M>) -> Option<&mut Self>;
}

impl<M> PropertyValueEnum<M> {
    /// This value as `T`, if that is its kind.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{property::values, PropertyValueEnum};
    ///
    /// let value = PropertyValueEnum::from(values::I32::new(42));
    ///
    /// assert_eq!(value.get::<values::I32>().map(|i| i.value), Some(42));
    /// assert_eq!(value.get::<values::F32>(), None);
    /// ```
    #[inline(always)]
    #[must_use]
    pub fn get<T: FromValue<M>>(&self) -> Option<&T> {
        T::from_value(self)
    }

    /// See [`PropertyValueEnum::get`].
    #[inline(always)]
    #[must_use]
    pub fn get_mut<T: FromValue<M>>(&mut self) -> Option<&mut T> {
        T::from_value_mut(self)
    }
}
