use crate::core::animation::asset::compressed::read::AnimationFlags;
use crate::core::animation::asset::error_metric::ErrorMetric;
use crate::core::animation::AnimationAsset;
use glam::Vec3;

mod frame;
mod read;
mod write;

mod primitive;

pub use frame::*;
pub use primitive::*;

use super::{Frame, FrameValue, TimedValue};

#[derive(Clone, Debug)]
pub struct Compressed {
    pub flags: AnimationFlags,
    pub duration: f32,
    pub fps: f32,

    pub rotation_error_metric: ErrorMetric,
    pub translation_error_metric: ErrorMetric,
    pub scale_error_metric: ErrorMetric,

    pub translation_min: Vec3,
    pub translation_max: Vec3,

    pub scale_min: Vec3,
    pub scale_max: Vec3,

    pub jump_cache_count: usize,
    pub compressed_frames: Vec<CompressedFrame>,
    pub jump_caches: Vec<u8>,
    pub joints: Vec<u32>,
}

impl From<Compressed> for AnimationAsset {
    fn from(val: Compressed) -> Self {
        AnimationAsset::Compressed(val)
    }
}

// TODO (alan): make a trait to abstract over compressed/uncompressed animations
impl Compressed {
    /// Returns a lazily-decompressed frame iterator
    pub fn frames(&self) -> impl Iterator<Item = Frame> + use<'_> {
        self.compressed_frames
            .iter()
            .map(|f| self.decompress_frame(f))
    }

    pub fn decompress_frame(&self, frame: &CompressedFrame) -> Frame {
        match frame.transform_type() {
            TransformType::Rotation => Frame {
                joint: frame.joint_id,
                value: FrameValue::Rotation(TimedValue::new(
                    CompressedTime::new(frame.time).decompress(self.duration),
                    CompressedQuat::new(frame.value).decompress(),
                )),
            },
            TransformType::Translation => Frame {
                joint: frame.joint_id,
                value: FrameValue::Translation(TimedValue::new(
                    CompressedTime::new(frame.time).decompress(self.duration),
                    CompressedVec3::new(frame.value)
                        .decompress(self.translation_min, self.translation_max),
                )),
            },
            TransformType::Scale => Frame {
                joint: frame.joint_id,
                value: FrameValue::Scale(TimedValue::new(
                    CompressedTime::new(frame.time).decompress(self.duration),
                    CompressedVec3::new(frame.value).decompress(self.scale_min, self.scale_max),
                )),
            },
        }
    }
}
