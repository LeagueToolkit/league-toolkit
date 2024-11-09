use crate::core::meta::traits::{PropertyValue, ReadProperty, WriteProperty};
use io_ext::{ReaderExt, WriterExt};

macro_rules! impl_prim {
    ($name:tt, $rust:tt, [$($derive:tt),*], $method:ident $(::<$endian:ident>)?, $($write_value:tt)*) => {
        #[repr(transparent)]
        #[derive(Clone, Debug, PartialEq, $($derive),*)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub struct $name(pub $rust);

        impl PropertyValue for $name {
            fn size_no_header(&self) -> usize {
                core::mem::size_of::<$rust>()
            }
        }

        impl ReadProperty for $name {
            fn from_reader<R: std::io::Read + ?Sized>(
                reader: &mut R,
                _legacy: bool,
            ) -> Result<Self, crate::core::meta::ParseError> {
                Ok(Self(paste::paste!(reader.[<read_ $method>]::<$($endian,)*>()?)))
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
    };
}

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use glam::{Mat4, Vec2, Vec3, Vec4};
use league_primitives::Color;

// A "primitive" in this case is just a PropertyValue that just encapsulates
// a single struct/rust primitive.

impl_prim!(BoolValue, bool, [Eq, Hash], bool, 0);

// https://github.com/LeagueToolkit/league-toolkit/pull/6#discussion_r1809366173
// > Afaik this is leftover from before bitfield support was added to league.
// > This type is also not a primitive, meaning it can't be used as a key for map.
// - moonshadow
impl_prim!(BitBoolValue, bool, [Eq, Hash], bool, 0);

impl_prim!(I8Value, i8, [Eq, Hash], i8, 0);
impl_prim!(U8Value, u8, [Eq, Hash], u8, 0);

impl_prim!(I16Value, i16, [Eq, Hash], i16::<LE>, 0);
impl_prim!(U16Value, u16, [Eq, Hash], u16::<LE>, 0);

impl_prim!(I32Value, i32, [Eq, Hash], i32::<LE>, 0);
impl_prim!(U32Value, u32, [Eq, Hash], u32::<LE>, 0);

impl_prim!(I64Value, i64, [Eq, Hash], i64::<LE>, 0);
impl_prim!(U64Value, u64, [Eq, Hash], u64::<LE>, 0);

impl_prim!(F32Value, f32, [], f32::<LE>, 0);

impl_prim!(Vector2Value, Vec2, [], vec2::<LE>, 0);
impl_prim!(Vector3Value, Vec3, [], vec3::<LE>, 0);
impl_prim!(Vector4Value, Vec4, [], vec4::<LE>, 0);
impl_prim!(Matrix44Value, Mat4, [], mat4_row_major::<LE>, 0);

type ColorU8 = Color<u8>;
impl_prim!(ColorValue, ColorU8, [], color_u8, 0.as_ref());
impl_prim!(HashValue, u32, [Eq, Hash], u32::<LE>, 0);
impl_prim!(WadChunkLinkValue, u64, [Eq, Hash], u64::<LE>, 0);
impl_prim!(ObjectLinkValue, u32, [Eq, Hash], u32::<LE>, 0);
