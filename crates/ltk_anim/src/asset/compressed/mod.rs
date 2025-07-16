use crate::core::animation::asset::compressed::frame::Frame;
use crate::core::animation::asset::compressed::read::AnimationFlags;
use crate::core::animation::asset::error_metric::ErrorMetric;
use crate::core::animation::AnimationAsset;
use glam::Vec3;

mod frame;
mod read;
mod write;

#[derive(Clone, Debug)]
pub struct Compressed {
    flags: AnimationFlags,
    duration: f32,
    fps: f32,

    rotation_error_metric: ErrorMetric,
    translation_error_metric: ErrorMetric,
    scale_error_metric: ErrorMetric,

    translation_min: Vec3,
    translation_max: Vec3,

    scale_min: Vec3,
    scale_max: Vec3,

    jump_cache_count: usize,
    frames: Vec<Frame>,
    jump_caches: Vec<u8>,
    joints: Vec<u32>,
}

impl Into<AnimationAsset> for Compressed {
    fn into(self) -> AnimationAsset {
        AnimationAsset::Compressed(self)
    }
}
