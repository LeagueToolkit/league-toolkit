/// The storage type of a [`Value`](crate::Value) in an inibin file.
///
/// The discriminant is the bit index of the storage type in the `flags` field of the file header.
/// A file stores the values of one storage type together, in discriminant order.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ValueKind {
    /// A signed 32-bit integer.
    I32 = 0,
    /// A 32-bit float.
    F32 = 1,
    /// An unsigned fixed-point number in one byte. The value is the byte divided by 10, from
    /// `0.0` to `25.5`.
    Fixed = 2,
    /// A signed 16-bit integer.
    I16 = 3,
    /// An unsigned 8-bit integer.
    U8 = 4,
    /// A 1-bit unsigned integer, 0 or 1. A file packs 8 values into one byte.
    U1 = 5,
    /// 3 unsigned fixed-point numbers, one byte each.
    FixedVec3 = 6,
    /// 3 32-bit floats.
    Vec3 = 7,
    /// 2 unsigned fixed-point numbers, one byte each.
    FixedVec2 = 8,
    /// 2 32-bit floats.
    Vec2 = 9,
    /// 4 unsigned fixed-point numbers, one byte each.
    FixedVec4 = 10,
    /// 4 32-bit floats.
    Vec4 = 11,
    /// A NUL-terminated string.
    String = 12,
    /// A signed 64-bit integer.
    I64 = 13,
}

impl ValueKind {
    /// Every storage type, in file order.
    pub const ALL: [ValueKind; 14] = [
        ValueKind::I32,
        ValueKind::F32,
        ValueKind::Fixed,
        ValueKind::I16,
        ValueKind::U8,
        ValueKind::U1,
        ValueKind::FixedVec3,
        ValueKind::Vec3,
        ValueKind::FixedVec2,
        ValueKind::Vec2,
        ValueKind::FixedVec4,
        ValueKind::Vec4,
        ValueKind::String,
        ValueKind::I64,
    ];

    /// Returns the bit of the storage type in the `flags` field of the file header.
    #[inline]
    #[must_use]
    pub const fn flag(self) -> u16 {
        1 << self as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_lists_kinds_in_discriminant_order() {
        for (index, kind) in ValueKind::ALL.into_iter().enumerate() {
            assert_eq!(kind as usize, index);
        }
    }

    #[test]
    fn flag_returns_header_bit() {
        assert_eq!(ValueKind::I32.flag(), 0x0001);
        assert_eq!(ValueKind::String.flag(), 0x1000);
        assert_eq!(ValueKind::I64.flag(), 0x2000);
    }
}
