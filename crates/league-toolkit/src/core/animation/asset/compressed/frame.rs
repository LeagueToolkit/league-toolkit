use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::core::animation::asset::{Frame, TimedValue};

use super::{CompressedQuat, CompressedVec3};

#[derive(Clone, Debug)]
#[repr(C, packed)]
pub struct CompressedFrame {
    pub time: u16,
    pub joint_id: u16,
    pub value: [u16; 3],
}

impl CompressedFrame {
    pub fn joint_id(&self) -> u16 {
        self.joint_id & 0x3fff
    }
    pub fn transform_type(&self) -> TransformType {
        TransformType::try_from_primitive((self.joint_id >> 14) as u8)
            .expect("invalid transform type")
    }
}

#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum TransformType {
    Rotation = 0,
    Translation = 1,
    Scale = 2,
}
