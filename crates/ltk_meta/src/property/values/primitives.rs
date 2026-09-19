use crate::{
    property::Kind,
    traits::{PropertyExt, PropertyValueExt, ReadProperty, WriteProperty},
};
use ltk_hash::{BinHash, ReadBytesExt as _, WadHash, WriteBytesExt as _};
use ltk_io_ext::{ReaderExt, WriterExt};

macro_rules! impl_prim {
    ($name:tt, $rust:tt, [$($derive:tt),*], $method:ident $(::<$endian:ident>)?) => {
        impl_prim!($name, $rust, [$( $derive ),*], $method $(::< $endian >)?, value);
    };
    ($name:tt, $rust:tt, ($new_arg:ty), [$($derive:tt),*], $method:ident $(::<$endian:ident>)?) => {
        impl_prim!($name, $rust, ($new_arg), [$( $derive ),*], $method $(::< $endian >)?, value);
    };
    ($name:tt, $rust:tt, [$($derive:tt),*], $method:ident $(::<$endian:ident>)?, $($write_value:tt)*) => {
        impl_prim!($name, $rust, ($rust), [$( $derive ),*], $method $(::< $endian >)?, $($write_value)*);
    };
    ($name:tt, $rust:tt, ($new_arg:ty), [$($derive:tt),*], $method:ident $(::<$endian:ident>)?, $($write_value:tt)*) => {
        #[derive(Clone, Debug, PartialEq, Default, $($derive),*)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize)
        )]
        pub struct $name {
            pub value: $rust,
        }

        impl $name {
            #[inline(always)]
            #[must_use]
            pub fn new(value: $new_arg) -> Self {
                Self { value: value.into() }
            }
        }

        impl PropertyExt for $name {
            fn size_no_header(&self) -> usize {
                core::mem::size_of::<$rust>()
            }
        }

        impl PropertyValueExt for $name {
            const KIND: Kind = Kind::$name;
        }

        impl ReadProperty for $name {
            fn from_reader<R: std::io::Read + ?Sized>(
                reader: &mut R,
                _legacy: bool,
            ) -> Result<Self, crate::Error> {
                Ok(Self {
                    value: paste::paste!(reader.[<read_ $method>]::<$($endian,)*>()?),
                })
            }
        }
        impl WriteProperty for $name {
            fn to_writer<W: std::io::Write + std::io::Seek + ?Sized>(
                &self,
                writer: &mut W,
                _legacy: bool,
            ) -> Result<(), std::io::Error> {
                paste::paste!(writer.[<write_ $method>]::<$($endian,)*>(self.$($write_value)*))
            }
        }

        impl<S: Into<$rust>> From<S> for $name {
            fn from(value: S) -> Self {
                Self::new(value.into())
            }
        }

        impl AsRef<$rust> for $name {
            fn as_ref(&self) -> &$rust {
                &self.value
            }
        }

        impl std::ops::Deref for $name {
            type Target = $rust;

            fn deref(&self) -> &Self::Target {
                &self.value
            }
        }
    };
}

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_primitives::Color as ColorPrim;

// A "primitive" in this case is just a PropertyValue that just encapsulates
// a single struct/rust primitive.

impl_prim!(Bool, bool, [Eq, Hash], bool);

// https://github.com/LeagueToolkit/league-toolkit/pull/6#discussion_r1809366173
// > Afaik this is leftover from before bitfield support was added to league.
// > This type is also not a primitive, meaning it can't be used as a key for map.
// - moonshadow
impl_prim!(BitBool, bool, [Eq, Hash], bool);

impl_prim!(I8, i8, [Eq, Hash], i8);
impl_prim!(U8, u8, [Eq, Hash], u8);

impl_prim!(I16, i16, [Eq, Hash], i16::<LE>);
impl_prim!(U16, u16, [Eq, Hash], u16::<LE>);

impl_prim!(I32, i32, [Eq, Hash], i32::<LE>);
impl_prim!(U32, u32, [Eq, Hash], u32::<LE>);

impl_prim!(I64, i64, [Eq, Hash], i64::<LE>);
impl_prim!(U64, u64, [Eq, Hash], u64::<LE>);

impl_prim!(F32, f32, [], f32::<LE>);

impl_prim!(Vector2, Vec2, [], vec2::<LE>);
impl_prim!(Vector3, Vec3, [], vec3::<LE>);
impl_prim!(Vector4, Vec4, [], vec4::<LE>);
impl_prim!(Matrix44, Mat4, [], mat4_row_major::<LE>);

type ColorU8 = ColorPrim<u8>;
impl_prim!(Color, ColorU8, [], color_u8, value.as_ref());
impl_prim!(
    Hash,
    BinHash,
    (impl Into<BinHash>),
    [Eq, Hash],
    bin_hash::<LE>
);
impl_prim!(
    WadChunkLink,
    WadHash,
    (impl Into<WadHash>),
    [Eq, Hash],
    wad_hash::<LE>
);
impl_prim!(
    ObjectLink,
    BinHash,
    (impl Into<BinHash>),
    [Eq, Hash],
    bin_hash::<LE>
);
