use std::io;

use num_enum::{IntoPrimitive, TryFromPrimitive};

#[derive(Clone, Debug)]
#[repr(C, packed)]
pub struct CompressedFrame {
    pub time: u16,
    joint_id: u16,
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

    pub fn read<R: io::Read + ?Sized>(read: &mut R) -> io::Result<Self> {
        use byteorder::{ReadBytesExt as _, LE};
        Ok(Self {
            time: read.read_u16::<LE>()?,
            joint_id: read.read_u16::<LE>()?,
            value: [
                read.read_u16::<LE>()?,
                read.read_u16::<LE>()?,
                read.read_u16::<LE>()?,
            ],
        })
    }
}

#[derive(TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum TransformType {
    Rotation = 0,
    Translation = 1,
    Scale = 2,
}
