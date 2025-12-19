//! Uncompressed animation asset (r3d2anmd)
use crate::AnimationAsset;
use glam::{Quat, Vec3};
use std::collections::HashMap;

mod read;
mod write;

/// An uncompressed animation frame for a single joint
#[derive(Clone, Copy, Debug, Default)]
pub struct UncompressedFrame {
    /// Index into the vector palette for translation
    pub translation_id: u16,
    /// Index into the vector palette for scale
    pub scale_id: u16,
    /// Index into the quaternion palette for rotation
    pub rotation_id: u16,
}

/// Uncompressed animation asset (`r3d2anmd` format)
///
/// Supports versions 3 (legacy), 4, and 5.
/// Uses palette-based storage where frames reference indices into
/// shared vector and quaternion arrays.
#[derive(Clone, Debug)]
pub struct Uncompressed {
    /// Duration of the animation in seconds
    pub(crate) duration: f32,
    /// Frames per second
    pub(crate) fps: f32,
    /// Total number of frames
    pub(crate) frame_count: usize,
    /// Shared vector palette (translations and scales)
    pub(crate) vector_palette: Vec<Vec3>,
    /// Shared quaternion palette (rotations)
    pub(crate) quat_palette: Vec<Quat>,
    /// Frames indexed by joint hash -> frame array
    pub(crate) joint_frames: HashMap<u32, Vec<UncompressedFrame>>,
}

impl Uncompressed {
    /// Creates a new uncompressed animation
    pub fn new(
        fps: f32,
        vector_palette: Vec<Vec3>,
        quat_palette: Vec<Quat>,
        joint_frames: HashMap<u32, Vec<UncompressedFrame>>,
    ) -> Self {
        let frame_count = joint_frames.values().next().map(|f| f.len()).unwrap_or(0);
        let duration = frame_count as f32 / fps;
        Self {
            duration,
            fps,
            frame_count,
            vector_palette,
            quat_palette,
            joint_frames,
        }
    }
    /// Returns the duration of the animation in seconds
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Returns the frames per second
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Returns the total number of frames
    pub fn frame_count(&self) -> usize {
        self.frame_count
    }

    /// Returns the vector palette (translations and scales)
    pub fn vector_palette(&self) -> &[Vec3] {
        &self.vector_palette
    }

    /// Returns the quaternion palette (rotations)
    pub fn quat_palette(&self) -> &[Quat] {
        &self.quat_palette
    }

    /// Returns the joint frames map
    pub fn joint_frames(&self) -> &HashMap<u32, Vec<UncompressedFrame>> {
        &self.joint_frames
    }

    /// Returns the joint hashes
    pub fn joint_hashes(&self) -> impl Iterator<Item = &u32> {
        self.joint_frames.keys()
    }

    /// Gets the frame data for a specific joint
    pub fn get_joint_frames(&self, joint_hash: u32) -> Option<&[UncompressedFrame]> {
        self.joint_frames.get(&joint_hash).map(|v| v.as_slice())
    }

    /// Evaluates the animation at the given frame index for a joint
    pub fn evaluate_frame(&self, joint_hash: u32, frame_id: usize) -> Option<(Quat, Vec3, Vec3)> {
        let frames = self.joint_frames.get(&joint_hash)?;
        let frame = frames.get(frame_id)?;

        let rotation = self.quat_palette.get(frame.rotation_id as usize)?;
        let translation = self.vector_palette.get(frame.translation_id as usize)?;
        let scale = self.vector_palette.get(frame.scale_id as usize)?;

        Some((*rotation, *translation, *scale))
    }
}

impl From<Uncompressed> for AnimationAsset {
    fn from(val: Uncompressed) -> Self {
        AnimationAsset::Uncompressed(val)
    }
}
