use glam::{Vec2, Vec3, Vec4};

use crate::value_flags::ValueFlags;

/// Typed value stored in an inibin set.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I32(i32),
    F32(f32),
    /// Packed float stored as raw byte. Use [`Value::as_f32`] to get the decoded `f32` value (`byte * 0.1`, range 0.0–25.5).
    U8(u8),
    I16(i16),
    I8(u8),
    Bool(bool),
    /// Packed float triple stored as raw bytes. Use [`Value::as_vec3`] to decode.
    Vec3U8([u8; 3]),
    Vec3F32(Vec3),
    /// Packed float pair stored as raw bytes. Use [`Value::as_vec2`] to decode.
    Vec2U8([u8; 2]),
    Vec2F32(Vec2),
    /// Packed float quad stored as raw bytes. Use [`Value::as_vec4`] to decode.
    Vec4U8([u8; 4]),
    Vec4F32(Vec4),
    String(String),
    I64(i64),
}

impl Value {
    /// Returns the [`ValueFlags`] variant this value belongs to.
    pub fn flags(&self) -> ValueFlags {
        match self {
            Value::I32(_) => ValueFlags::INT32_LIST,
            Value::F32(_) => ValueFlags::F32_LIST,
            Value::U8(_) => ValueFlags::U8_LIST,
            Value::I16(_) => ValueFlags::INT16_LIST,
            Value::I8(_) => ValueFlags::INT8_LIST,
            Value::Bool(_) => ValueFlags::BIT_LIST,
            Value::Vec3U8(_) => ValueFlags::VEC3_U8_LIST,
            Value::Vec3F32(_) => ValueFlags::VEC3_F32_LIST,
            Value::Vec2U8(_) => ValueFlags::VEC2_U8_LIST,
            Value::Vec2F32(_) => ValueFlags::VEC2_F32_LIST,
            Value::Vec4U8(_) => ValueFlags::VEC4_U8_LIST,
            Value::Vec4F32(_) => ValueFlags::VEC4_F32_LIST,
            Value::String(_) => ValueFlags::STRING_LIST,
            Value::I64(_) => ValueFlags::INT64_LIST,
        }
    }

    /// Returns the value as `f32`, handling both [`F32`](Value::F32) and packed [`U8`](Value::U8) variants.
    ///
    /// For `U8`: returns `byte * 0.1` (range 0.0–25.5).
    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Value::F32(v) => Some(*v),
            Value::U8(v) => Some(*v as f32 * 0.1),
            _ => None,
        }
    }

    /// Returns the value as [`Vec2`], handling both [`Vec2F32`](Value::Vec2F32) and packed [`Vec2U8`](Value::Vec2U8) variants.
    pub fn as_vec2(&self) -> Option<Vec2> {
        match self {
            Value::Vec2F32(v) => Some(*v),
            Value::Vec2U8([x, y]) => Some(Vec2::new(*x as f32 * 0.1, *y as f32 * 0.1)),
            _ => None,
        }
    }

    /// Returns the value as [`Vec3`], handling both [`Vec3F32`](Value::Vec3F32) and packed [`Vec3U8`](Value::Vec3U8) variants.
    pub fn as_vec3(&self) -> Option<Vec3> {
        match self {
            Value::Vec3F32(v) => Some(*v),
            Value::Vec3U8([x, y, z]) => {
                Some(Vec3::new(*x as f32 * 0.1, *y as f32 * 0.1, *z as f32 * 0.1))
            }
            _ => None,
        }
    }

    /// Returns the value as [`Vec4`], handling both [`Vec4F32`](Value::Vec4F32) and packed [`Vec4U8`](Value::Vec4U8) variants.
    pub fn as_vec4(&self) -> Option<Vec4> {
        match self {
            Value::Vec4F32(v) => Some(*v),
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
