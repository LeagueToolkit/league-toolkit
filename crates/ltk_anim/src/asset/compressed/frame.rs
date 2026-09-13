use crate::asset::{self, AssetParseError::InvalidField};
use num_enum::TryFromPrimitive;

/// One key of one transform of one joint.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    time: u16,
    joint_id: u16,
    transform_type: TransformType,
    value: [u16; 3],
}

impl Frame {
    /// The size of a frame in a file, in bytes.
    pub const SIZE: usize = 10;

    /// Decodes a frame from its little-endian bytes.
    ///
    /// Bytes 0..2 hold the time and bytes 4..10 the value. Bytes 2..4 hold the joint id in
    /// the low 14 bits and the transform type in the top 2.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidField`] when the transform type bits are `3`, which name no
    /// [`TransformType`].
    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> asset::Result<Self> {
        let [t0, t1, j0, j1, v0, v1, v2, v3, v4, v5] = bytes;
        let joint_id = u16::from_le_bytes([j0, j1]);
        let transform_bits = (joint_id >> 14) as u8;
        let transform_type = TransformType::try_from_primitive(transform_bits)
            .map_err(|_| InvalidField("frame transform type", transform_bits.to_string()))?;

        Ok(Self {
            time: u16::from_le_bytes([t0, t1]),
            joint_id: joint_id & 0x3fff,
            transform_type,
            value: [
                u16::from_le_bytes([v0, v1]),
                u16::from_le_bytes([v2, v3]),
                u16::from_le_bytes([v4, v5]),
            ],
        })
    }

    pub fn time(&self) -> u16 {
        self.time
    }
    pub fn value(&self) -> [u16; 3] {
        self.value
    }
    pub fn joint_id(&self) -> u16 {
        self.joint_id
    }
    pub fn transform_type(&self) -> TransformType {
        self.transform_type
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum TransformType {
    Rotation = 0,
    Translation = 1,
    Scale = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_splits_joint_id_and_transform_type() {
        let frame = Frame::from_bytes([0x34, 0x12, 0x05, 0x40, 1, 0, 2, 0, 3, 0]).unwrap();

        assert_eq!(frame.time(), 0x1234);
        assert_eq!(frame.joint_id(), 5);
        assert_eq!(frame.transform_type(), TransformType::Translation);
        assert_eq!(frame.value(), [1, 2, 3]);
    }

    #[test]
    fn from_bytes_rejects_transform_type_3() {
        let result = Frame::from_bytes([0, 0, 0x05, 0xC0, 0, 0, 0, 0, 0, 0]);

        assert!(matches!(
            result,
            Err(InvalidField("frame transform type", value)) if value == "3"
        ));
    }
}
