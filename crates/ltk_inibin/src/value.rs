use glam::{Vec2, Vec3, Vec4};

use crate::value_kind::ValueKind;

/// Typed value stored in an inibin set.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I32(i32),
    F32(f32),
    /// Packed float stored as raw byte. Use [`Value::u8_as_f32`] to get the decoded `f32` value (`byte * 0.1`, range 0.0–25.5).
    U8(u8),
    I16(i16),
    I8(u8),
    Bool(bool),
    /// Packed float triple stored as raw bytes. Use [`Value::vec3_u8_as_f32`] to decode.
    Vec3U8([u8; 3]),
    Vec3F32(Vec3),
    /// Packed float pair stored as raw bytes. Use [`Value::vec2_u8_as_f32`] to decode.
    Vec2U8([u8; 2]),
    Vec2F32(Vec2),
    /// Packed float quad stored as raw bytes. Use [`Value::vec4_u8_as_f32`] to decode.
    Vec4U8([u8; 4]),
    Vec4F32(Vec4),
    String(String),
    I64(i64),
}

impl Value {
    /// Returns the [`ValueKind`] variant this value belongs to.
    pub fn flags(&self) -> ValueKind {
        match self {
            Value::I32(_) => ValueKind::INT32_LIST,
            Value::F32(_) => ValueKind::F32_LIST,
            Value::U8(_) => ValueKind::U8_LIST,
            Value::I16(_) => ValueKind::INT16_LIST,
            Value::I8(_) => ValueKind::INT8_LIST,
            Value::Bool(_) => ValueKind::BIT_LIST,
            Value::Vec3U8(_) => ValueKind::VEC3_U8_LIST,
            Value::Vec3F32(_) => ValueKind::VEC3_F32_LIST,
            Value::Vec2U8(_) => ValueKind::VEC2_U8_LIST,
            Value::Vec2F32(_) => ValueKind::VEC2_F32_LIST,
            Value::Vec4U8(_) => ValueKind::VEC4_U8_LIST,
            Value::Vec4F32(_) => ValueKind::VEC4_F32_LIST,
            Value::String(_) => ValueKind::STRING_LIST,
            Value::I64(_) => ValueKind::INT64_LIST,
        }
    }

    /// Decode a [`U8`](Value::U8) packed float: `byte * 0.1` (range 0.0–25.5).
    pub fn u8_as_f32(&self) -> Option<f32> {
        match self {
            Value::U8(v) => Some(*v as f32 * 0.1),
            _ => None,
        }
    }

    /// Decode a [`Vec2U8`](Value::Vec2U8) packed float pair.
    pub fn vec2_u8_as_f32(&self) -> Option<Vec2> {
        match self {
            Value::Vec2U8([x, y]) => Some(Vec2::new(*x as f32 * 0.1, *y as f32 * 0.1)),
            _ => None,
        }
    }

    /// Decode a [`Vec3U8`](Value::Vec3U8) packed float triple.
    pub fn vec3_u8_as_f32(&self) -> Option<Vec3> {
        match self {
            Value::Vec3U8([x, y, z]) => {
                Some(Vec3::new(*x as f32 * 0.1, *y as f32 * 0.1, *z as f32 * 0.1))
            }
            _ => None,
        }
    }

    /// Decode a [`Vec4U8`](Value::Vec4U8) packed float quad.
    pub fn vec4_u8_as_f32(&self) -> Option<Vec4> {
        match self {
            Value::Vec4U8([x, y, z, w]) => Some(Vec4::new(
                *x as f32 * 0.1,
                *y as f32 * 0.1,
                *z as f32 * 0.1,
                *w as f32 * 0.1,
            )),
            _ => None,
        }
    }
}

// ── FromValue trait + From/extraction impls via macros ─────

/// Trait for extracting a typed value from an [`Value`] reference.
///
/// Enables the generic [`Inibin::get_as`](crate::Inibin::get_as) method:
/// ```
/// # use ltk_inibin::{Inibin, Value};
/// let mut inibin = Inibin::new();
/// inibin.insert(0x0001, 42i32);
/// let v: Option<i32> = inibin.get_as(0x0001);
/// assert_eq!(v, Some(42));
/// ```
pub trait FromValue<'a>: Sized {
    /// Try to extract from an [`Value`] reference. Returns `None` on type mismatch.
    fn from_inibin_value(value: &'a Value) -> Option<Self>;
}

/// Generates `From<$ty>` and `FromValue` for Copy types with a direct variant mapping.
macro_rules! impl_value_conversion {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for Value {
            fn from(v: $ty) -> Self {
                Value::$variant(v)
            }
        }

        impl FromValue<'_> for $ty {
            fn from_inibin_value(value: &Value) -> Option<Self> {
                match value {
                    Value::$variant(v) => Some(*v),
                    _ => None,
                }
            }
        }
    };
}

impl_value_conversion!(i32, I32);
impl_value_conversion!(f32, F32);
impl_value_conversion!(i16, I16);
impl_value_conversion!(bool, Bool);
impl_value_conversion!(i64, I64);
impl_value_conversion!(Vec3, Vec3F32);
impl_value_conversion!(Vec2, Vec2F32);
impl_value_conversion!(Vec4, Vec4F32);

// u8 maps to I8 (raw byte) — not U8 (packed float)
impl FromValue<'_> for u8 {
    fn from_inibin_value(value: &Value) -> Option<Self> {
        match value {
            Value::I8(v) => Some(*v),
            _ => None,
        }
    }
}

// String types need special handling (not Copy)
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_owned())
    }
}

impl<'a> FromValue<'a> for &'a str {
    fn from_inibin_value(value: &'a Value) -> Option<Self> {
        match value {
            Value::String(v) => Some(v),
            _ => None,
        }
    }
}
