use crate::core::meta::traits::{PropertyValue, ReadProperty};

macro_rules! impl_prim {
    ($name:tt, $rust:tt, [$($derive:tt),*], $($method:tt)*) => {
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
            fn from_reader<R: std::io::Read>(
                reader: &mut R,
                _legacy: bool,
            ) -> Result<Self, crate::core::meta::ParseError> {
                Ok(Self(reader.$($method)*()?))
            }
        }
    };
}

use crate::core::primitives::Color;
use crate::util::ReaderExt;
use byteorder::{ReadBytesExt, LE};
use glam::{Mat4, Vec2, Vec3, Vec4};

// A "primitive" in this case is just a PropertyValue that just encapsulates
// a single struct/rust primitive.

impl_prim!(BoolValue, bool, [Eq, Hash], read_bool);

// https://github.com/LeagueToolkit/league-toolkit/pull/6#discussion_r1809366173
// > Afaik this is leftover from before bitfield support was added to league.
// > This type is also not a primitive, meaning it can't be used as a key for map.
// - moonshadow
impl_prim!(BitBoolValue, bool, [Eq, Hash], read_bool);

impl_prim!(I8Value, i8, [Eq, Hash], read_i8);
impl_prim!(U8Value, u8, [Eq, Hash], read_u8);

impl_prim!(I16Value, i16, [Eq, Hash], read_i16::<LE>);
impl_prim!(U16Value, u16, [Eq, Hash], read_u16::<LE>);

impl_prim!(I32Value, i32, [Eq, Hash], read_i32::<LE>);
impl_prim!(U32Value, u32, [Eq, Hash], read_u32::<LE>);

impl_prim!(I64Value, i64, [Eq, Hash], read_i64::<LE>);
impl_prim!(U64Value, u64, [Eq, Hash], read_u64::<LE>);

impl_prim!(F32Value, f32, [], read_f32::<LE>);

impl_prim!(Vector2Value, Vec2, [], read_vec2::<LE>);
impl_prim!(Vector3Value, Vec3, [], read_vec3::<LE>);
impl_prim!(Vector4Value, Vec4, [], read_vec4::<LE>);
impl_prim!(Matrix44Value, Mat4, [], read_mat4_row_major::<LE>);

type ColorU8 = Color<u8>;
impl_prim!(ColorValue, ColorU8, [], read_color_u8);
impl_prim!(HashValue, u32, [Eq, Hash], read_u32::<LE>);
impl_prim!(WadChunkLinkValue, u64, [Eq, Hash], read_u64::<LE>);
impl_prim!(ObjectLinkValue, u32, [Eq, Hash], read_u32::<LE>);
