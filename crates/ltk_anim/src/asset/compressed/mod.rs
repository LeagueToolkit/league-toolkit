use crate::{
    asset::{
        compressed::{frame::Frame, read::AnimationFlags},
        error_metric::ErrorMetric,
    },
    AnimationAsset,
};
use glam::Vec3;

mod frame;
mod read;
mod write;

#[derive(Clone, Debug)]
#[allow(dead_code)]
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

impl From<Compressed> for AnimationAsset {
    fn from(val: Compressed) -> Self {
        AnimationAsset::Compressed(val)
    }
}
