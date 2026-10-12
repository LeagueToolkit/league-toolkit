use std::fmt;

use glam::{Vec2, Vec3, Vec4};

use crate::{scan, ValueKind};

/// A value of an inibin file.
///
/// Each variant is one storage type of the file format. The ini compiler of the game selects the
/// storage type of each value per file. One property is a [`Value::F32`] in one file and a
/// [`Value::Fixed`] in another file. Some files store a vector as a [`Value::String`].
///
/// Each `to_*` method returns the value that the client reads. A `to_*` method converts from
/// every storage type that the matching client getter converts from.
///
/// ```
/// use ltk_ini::Value;
///
/// assert_eq!(Value::Fixed(15).to_f32(), Some(1.5));
/// assert_eq!(Value::Bool(true).to_i32(), Some(1));
/// assert_eq!(Value::from("0 .5 1").to_vec3(), Some(glam::vec3(0.0, 0.5, 1.0)));
/// ```
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    I32(i32),
    F32(f32),
    /// An unsigned fixed-point number. The value is the byte divided by 10. `Fixed(15)` is
    /// `1.5`.
    Fixed(u8),
    I16(i16),
    U8(u8),
    Bool(bool),
    /// 3 unsigned fixed-point numbers. `FixedVec3([0, 5, 10])` is `(0.0, 0.5, 1.0)`.
    FixedVec3([u8; 3]),
    Vec3(Vec3),
    /// 2 unsigned fixed-point numbers.
    FixedVec2([u8; 2]),
    Vec2(Vec2),
    /// 4 unsigned fixed-point numbers.
    FixedVec4([u8; 4]),
    Vec4(Vec4),
    String(String),
    I64(i64),
}

fn fixed_to_f32(fixed: u8) -> f32 {
    f32::from(fixed) / 10.0
}

/// Returns the fixed-point byte that is equal to `value`. Returns `None` if no fixed-point byte is
/// equal to `value`.
fn f32_to_fixed(value: f32) -> Option<u8> {
    let fixed = (value * 10.0).round();
    if !(0.0..=255.0).contains(&fixed) || value.is_sign_negative() {
        return None;
    }
    let fixed = fixed as u8;
    (fixed_to_f32(fixed) == value).then_some(fixed)
}

fn fixed_array<const N: usize>(value: [f32; N]) -> Option<[u8; N]> {
    let mut fixed = [0; N];
    for (fixed, component) in fixed.iter_mut().zip(value) {
        *fixed = f32_to_fixed(component)?;
    }
    Some(fixed)
}

impl Value {
    /// Returns the storage type of the value.
    #[must_use]
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::I32(_) => ValueKind::I32,
            Value::F32(_) => ValueKind::F32,
            Value::Fixed(_) => ValueKind::Fixed,
            Value::I16(_) => ValueKind::I16,
            Value::U8(_) => ValueKind::U8,
            Value::Bool(_) => ValueKind::Bool,
            Value::FixedVec3(_) => ValueKind::FixedVec3,
            Value::Vec3(_) => ValueKind::Vec3,
            Value::FixedVec2(_) => ValueKind::FixedVec2,
            Value::Vec2(_) => ValueKind::Vec2,
            Value::FixedVec4(_) => ValueKind::FixedVec4,
            Value::Vec4(_) => ValueKind::Vec4,
            Value::String(_) => ValueKind::String,
            Value::I64(_) => ValueKind::I64,
        }
    }

    /// Returns `value` in the smallest integer storage type that can store it.
    ///
    /// Returns a [`Value::Bool`] for 0 and 1. The ini compiler stores 0 and 1 as booleans.
    ///
    /// ```
    /// use ltk_ini::Value;
    ///
    /// assert_eq!(Value::compact_int(1), Value::Bool(true));
    /// assert_eq!(Value::compact_int(200), Value::U8(200));
    /// assert_eq!(Value::compact_int(-1), Value::I16(-1));
    /// assert_eq!(Value::compact_int(70_000), Value::I32(70_000));
    /// ```
    #[must_use]
    pub fn compact_int(value: i64) -> Self {
        if value == 0 || value == 1 {
            Value::Bool(value == 1)
        } else if let Ok(value) = u8::try_from(value) {
            Value::U8(value)
        } else if let Ok(value) = i16::try_from(value) {
            Value::I16(value)
        } else if let Ok(value) = i32::try_from(value) {
            Value::I32(value)
        } else {
            Value::I64(value)
        }
    }

    /// Returns a [`Value::Fixed`] if a fixed-point byte is equal to `value`. Otherwise returns a
    /// [`Value::F32`].
    ///
    /// ```
    /// use ltk_ini::Value;
    ///
    /// assert_eq!(Value::compact_float(1.5), Value::Fixed(15));
    /// assert_eq!(Value::compact_float(1.25), Value::F32(1.25));
    /// assert_eq!(Value::compact_float(-1.5), Value::F32(-1.5));
    /// ```
    #[must_use]
    pub fn compact_float(value: f32) -> Self {
        f32_to_fixed(value).map_or(Value::F32(value), Value::Fixed)
    }

    /// Returns a [`Value::FixedVec2`] if a fixed-point byte is equal to each component of
    /// `value`. Otherwise returns a [`Value::Vec2`].
    #[must_use]
    pub fn compact_vec2(value: Vec2) -> Self {
        fixed_array(value.to_array()).map_or(Value::Vec2(value), Value::FixedVec2)
    }

    /// Returns a [`Value::FixedVec3`] if a fixed-point byte is equal to each component of
    /// `value`. Otherwise returns a [`Value::Vec3`].
    #[must_use]
    pub fn compact_vec3(value: Vec3) -> Self {
        fixed_array(value.to_array()).map_or(Value::Vec3(value), Value::FixedVec3)
    }

    /// Returns a [`Value::FixedVec4`] if a fixed-point byte is equal to each component of
    /// `value`. Otherwise returns a [`Value::Vec4`].
    #[must_use]
    pub fn compact_vec4(value: Vec4) -> Self {
        fixed_array(value.to_array()).map_or(Value::Vec4(value), Value::FixedVec4)
    }

    /// Returns the string of a [`Value::String`]. Returns `None` for every other storage type.
    ///
    /// The [`Display`](fmt::Display) implementation converts every storage type to text.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(value) => Some(value),
            _ => None,
        }
    }

    /// Returns the value as the 64-bit integer that the client reads.
    ///
    /// Returns the number of an integer value. Returns the number of a float value, truncated
    /// toward zero. Returns 0 or 1 for a boolean. Returns the integer at the start of a string.
    /// Returns 0 if the string does not start with an integer. Returns `None` for a vector.
    #[must_use]
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Value::I64(value) => Some(*value),
            Value::I32(value) => Some(i64::from(*value)),
            Value::I16(value) => Some(i64::from(*value)),
            Value::U8(value) => Some(i64::from(*value)),
            Value::F32(value) => Some(*value as i64),
            Value::Fixed(fixed) => Some(fixed_to_f32(*fixed) as i64),
            Value::Bool(value) => Some(i64::from(*value)),
            Value::String(value) => Some(scan::atoi(value)),
            _ => None,
        }
    }

    /// Returns the value as the 32-bit integer that the client reads.
    ///
    /// Converts the value as [`to_i64`](Self::to_i64) does. Returns the low 32 bits of an integer
    /// that does not fit in an `i32`.
    #[must_use]
    pub fn to_i32(&self) -> Option<i32> {
        match self {
            Value::F32(value) => Some(*value as i32),
            _ => self.to_i64().map(|value| value as i32),
        }
    }

    /// Returns the value as the float that the client reads.
    ///
    /// Returns the number of a float value or an integer value. Returns 0.0 or 1.0 for a boolean.
    /// Returns the float at the start of a string. Returns 0.0 if the string does not start with
    /// a float. Returns `None` for a vector.
    #[must_use]
    pub fn to_f32(&self) -> Option<f32> {
        match self {
            Value::F32(value) => Some(*value),
            Value::Fixed(fixed) => Some(fixed_to_f32(*fixed)),
            Value::I64(value) => Some(*value as f32),
            Value::I32(value) => Some(*value as f32),
            Value::I16(value) => Some(f32::from(*value)),
            Value::U8(value) => Some(f32::from(*value)),
            Value::Bool(value) => Some(f32::from(u8::from(*value))),
            Value::String(value) => Some(scan::atof(value)),
            _ => None,
        }
    }

    /// Returns the value as the boolean that the client reads.
    ///
    /// Returns the value of a boolean. Returns `true` for a string that starts with `1`. Returns
    /// `false` for every other string. Returns `None` for every other storage type.
    #[must_use]
    pub fn to_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(value) => Some(*value),
            Value::String(value) => Some(value.starts_with('1')),
            _ => None,
        }
    }

    /// Returns the value as the 2 floats that the client reads.
    ///
    /// Returns the components of a [`Value::Vec2`] or a [`Value::FixedVec2`]. Returns the first 2
    /// floats of a string. Returns `None` if the string starts with fewer than 2 floats. Returns
    /// `None` for every other storage type.
    #[must_use]
    pub fn to_vec2(&self) -> Option<Vec2> {
        match self {
            Value::Vec2(value) => Some(*value),
            Value::FixedVec2(fixed) => Some(Vec2::from_array(fixed.map(fixed_to_f32))),
            Value::String(value) => scan::scan_floats(value).map(Vec2::from_array),
            _ => None,
        }
    }

    /// Returns the value as the 3 floats that the client reads.
    ///
    /// Returns the components of a [`Value::Vec3`] or a [`Value::FixedVec3`]. Returns the first 3
    /// floats of a string. Returns `None` if the string starts with fewer than 3 floats. Returns
    /// `None` for every other storage type.
    #[must_use]
    pub fn to_vec3(&self) -> Option<Vec3> {
        match self {
            Value::Vec3(value) => Some(*value),
            Value::FixedVec3(fixed) => Some(Vec3::from_array(fixed.map(fixed_to_f32))),
            Value::String(value) => scan::scan_floats(value).map(Vec3::from_array),
            _ => None,
        }
    }

    /// Returns the value as the 4 floats that the client reads.
    ///
    /// Returns the components of a [`Value::Vec4`] or a [`Value::FixedVec4`]. Returns the first 4
    /// floats of a string. Returns `None` if the string starts with fewer than 4 floats. Returns
    /// `None` for every other storage type.
    #[must_use]
    pub fn to_vec4(&self) -> Option<Vec4> {
        match self {
            Value::Vec4(value) => Some(*value),
            Value::FixedVec4(fixed) => Some(Vec4::from_array(fixed.map(fixed_to_f32))),
            Value::String(value) => scan::scan_floats(value).map(Vec4::from_array),
            _ => None,
        }
    }
}

fn fmt_components<const N: usize>(f: &mut fmt::Formatter<'_>, value: [f32; N]) -> fmt::Result {
    for (index, component) in value.into_iter().enumerate() {
        if index > 0 {
            f.write_str(" ")?;
        }
        write!(f, "{component}")?;
    }
    Ok(())
}

/// Formats the value as ini text.
///
/// Writes a boolean as `1` or `0`. Writes the components of a vector separated by one space.
/// Writes a float with the fewest digits that parse back to the same `f32`.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::I32(value) => write!(f, "{value}"),
            Value::F32(value) => write!(f, "{value}"),
            Value::Fixed(fixed) => write!(f, "{}", fixed_to_f32(*fixed)),
            Value::I16(value) => write!(f, "{value}"),
            Value::U8(value) => write!(f, "{value}"),
            Value::Bool(value) => write!(f, "{}", u8::from(*value)),
            Value::FixedVec3(fixed) => fmt_components(f, fixed.map(fixed_to_f32)),
            Value::Vec3(value) => fmt_components(f, value.to_array()),
            Value::FixedVec2(fixed) => fmt_components(f, fixed.map(fixed_to_f32)),
            Value::Vec2(value) => fmt_components(f, value.to_array()),
            Value::FixedVec4(fixed) => fmt_components(f, fixed.map(fixed_to_f32)),
            Value::Vec4(value) => fmt_components(f, value.to_array()),
            Value::String(value) => f.write_str(value),
            Value::I64(value) => write!(f, "{value}"),
        }
    }
}

macro_rules! impl_from {
    ($($ty:ty => $variant:ident),* $(,)?) => {
        $(
            impl From<$ty> for Value {
                fn from(value: $ty) -> Self {
                    Value::$variant(value)
                }
            }
        )*
    };
}

impl_from! {
    i32 => I32,
    f32 => F32,
    i16 => I16,
    u8 => U8,
    bool => Bool,
    Vec3 => Vec3,
    Vec2 => Vec2,
    Vec4 => Vec4,
    String => String,
    i64 => I64,
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use glam::{vec2, vec3, vec4};

    use super::*;

    #[test]
    fn compact_int_returns_smallest_integer_kind() {
        assert_eq!(Value::compact_int(0), Value::Bool(false));
        assert_eq!(Value::compact_int(1), Value::Bool(true));
        assert_eq!(Value::compact_int(2), Value::U8(2));
        assert_eq!(Value::compact_int(255), Value::U8(255));
        assert_eq!(Value::compact_int(256), Value::I16(256));
        assert_eq!(Value::compact_int(-1), Value::I16(-1));
        assert_eq!(Value::compact_int(32_768), Value::I32(32_768));
        assert_eq!(Value::compact_int(1 << 40), Value::I64(1 << 40));
    }

    #[test]
    fn compact_float_returns_fixed_for_every_fixed_byte() {
        for fixed in 0..=u8::MAX {
            let value = fixed_to_f32(fixed);
            assert_eq!(Value::compact_float(value), Value::Fixed(fixed));
        }
    }

    #[test]
    fn compact_float_returns_f32_without_equal_fixed_byte() {
        for value in [0.25, 25.6, -0.5, -0.0, 0.500_001, f32::NAN, f32::INFINITY] {
            assert_eq!(Value::compact_float(value).kind(), ValueKind::F32);
        }
    }

    #[test]
    fn compact_vec_returns_fixed_if_every_component_has_fixed_byte() {
        assert_eq!(
            Value::compact_vec2(vec2(0.5, 1.0)),
            Value::FixedVec2([5, 10])
        );
        assert_eq!(
            Value::compact_vec3(vec3(0.0, 25.5, 0.1)),
            Value::FixedVec3([0, 255, 1])
        );
        assert_eq!(
            Value::compact_vec4(vec4(1.0, 1.0, 1.0, 0.25)),
            Value::Vec4(vec4(1.0, 1.0, 1.0, 0.25))
        );
    }

    #[test]
    fn to_i32_converts_like_client_getter() {
        assert_eq!(Value::I64(7).to_i32(), Some(7));
        assert_eq!(Value::I64(0x1_0000_0002).to_i32(), Some(2));
        assert_eq!(Value::I16(-3).to_i32(), Some(-3));
        assert_eq!(Value::U8(200).to_i32(), Some(200));
        assert_eq!(Value::F32(-2.9).to_i32(), Some(-2));
        assert_eq!(Value::Fixed(19).to_i32(), Some(1));
        assert_eq!(Value::Bool(true).to_i32(), Some(1));
        assert_eq!(Value::from("12 34").to_i32(), Some(12));
        assert_eq!(Value::from("Yes").to_i32(), Some(0));
        assert_eq!(Value::Vec2(vec2(1.0, 2.0)).to_i32(), None);
    }

    #[test]
    fn to_f32_converts_like_client_getter() {
        assert_eq!(Value::F32(0.25).to_f32(), Some(0.25));
        assert_eq!(Value::Fixed(3).to_f32(), Some(0.3));
        assert_eq!(Value::I32(-5).to_f32(), Some(-5.0));
        assert_eq!(Value::Bool(false).to_f32(), Some(0.0));
        assert_eq!(Value::from("-.05").to_f32(), Some(-0.05));
        assert_eq!(Value::from("TRUE").to_f32(), Some(0.0));
        assert_eq!(Value::FixedVec2([1, 2]).to_f32(), None);
    }

    #[test]
    fn to_bool_converts_like_client_getter() {
        assert_eq!(Value::Bool(true).to_bool(), Some(true));
        assert_eq!(Value::from("1").to_bool(), Some(true));
        assert_eq!(Value::from("true").to_bool(), Some(false));
        assert_eq!(Value::from("").to_bool(), Some(false));
        assert_eq!(Value::U8(2).to_bool(), None);
    }

    #[test]
    fn to_vec_converts_like_client_getter() {
        assert_eq!(Value::FixedVec2([5, 10]).to_vec2(), Some(vec2(0.5, 1.0)));
        assert_eq!(
            Value::FixedVec3([0, 20, 0]).to_vec3(),
            Some(vec3(0.0, 2.0, 0.0))
        );
        assert_eq!(
            Value::from("1 .5 0 0").to_vec4(),
            Some(vec4(1.0, 0.5, 0.0, 0.0))
        );
        assert_eq!(Value::from("1 .5 0 0").to_vec2(), Some(vec2(1.0, 0.5)));
        assert_eq!(Value::from("1 .5").to_vec3(), None);
        assert_eq!(Value::Vec3(vec3(1.0, 2.0, 3.0)).to_vec2(), None);
        assert_eq!(Value::F32(1.0).to_vec2(), None);
    }

    #[test]
    fn display_writes_ini_text() {
        assert_eq!(Value::I16(-80).to_string(), "-80");
        assert_eq!(Value::F32(1000.0).to_string(), "1000");
        assert_eq!(Value::Fixed(15).to_string(), "1.5");
        assert_eq!(Value::Bool(true).to_string(), "1");
        assert_eq!(Value::FixedVec3([0, 20, 5]).to_string(), "0 2 0.5");
        assert_eq!(Value::Vec2(vec2(0.25, -1.0)).to_string(), "0.25 -1");
        assert_eq!(Value::from("Simple").to_string(), "Simple");
    }

    #[test]
    fn display_text_of_fixed_converts_back_to_same_f32() {
        for fixed in 0..=u8::MAX {
            let text = Value::Fixed(fixed).to_string();
            assert_eq!(
                Value::from(text.as_str()).to_f32(),
                Value::Fixed(fixed).to_f32()
            );
        }
    }
}
