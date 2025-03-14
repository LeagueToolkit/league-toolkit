use crate::core::animation::asset::compressed::frame::Frame;
use crate::core::animation::asset::compressed::read::AnimationFlags;
use crate::core::animation::asset::error_metric::ErrorMetric;
use crate::core::animation::AnimationAsset;
use glam::Vec3;

mod frame;
mod read;
mod write;

mod primitive;
pub use primitive::*;

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
    pub frames: Vec<Frame>,
    pub jump_caches: Vec<u8>,
    pub joints: Vec<u32>,
}

impl From<Compressed> for AnimationAsset {
    fn from(val: Compressed) -> Self {
        AnimationAsset::Compressed(val)
    }
}
