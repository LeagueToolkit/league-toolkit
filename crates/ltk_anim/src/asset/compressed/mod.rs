use crate::{
    asset::{
        compressed::{
            evaluate::{
                compress_time, decompress_vector3, HotFrameEvaluator, JointHotFrame, JumpFrameU16,
                JumpFrameU32, QuaternionHotFrame, VectorHotFrame,
            },
            frame::Frame,
            read::AnimationFlags,
        },
        error_metric::ErrorMetric,
        Animation,
    },
    quantized, AnimationAsset,
};
use glam::{Quat, Vec3};
use std::borrow::Cow;
use std::collections::HashMap;

mod evaluate;
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

impl Compressed {
    /// Returns the animation duration in seconds
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Returns the animation FPS
    pub fn fps(&self) -> f32 {
        self.fps
    }

    /// Returns the joint hashes
    pub fn joints(&self) -> &[u32] {
        &self.joints
    }

    /// Returns the number of joints
    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    /// Evaluates the animation at the given time
    ///
    /// Returns a map of joint hash -> (rotation, translation, scale)
    pub fn evaluate(&self, time: f32) -> HashMap<u32, (Quat, Vec3, Vec3)> {
        let time = time.clamp(0.0, self.duration);
        let parametrized = self
            .flags
            .contains(AnimationFlags::UseKeyframeParametrization);

        let mut evaluator = HotFrameEvaluator::new(self.joints.len());
        self.initialize_hot_frame_evaluator(&mut evaluator, time);

        let compressed_time = compress_time(time, self.duration);

        self.joints
            .iter()
            .enumerate()
            .map(|(id, &hash)| {
                (
                    hash,
                    evaluator.hot_frames[id].sample(compressed_time, parametrized),
                )
            })
            .collect()
    }

    /// Initializes the hot frame evaluator from jump caches
    fn initialize_hot_frame_evaluator(&self, evaluator: &mut HotFrameEvaluator, time: f32) {
        if self.jump_cache_count == 0 {
            return;
        }

        // Get cache id based on time
        let jump_cache_id = ((self.jump_cache_count as f32 * (time / self.duration)) as usize)
            .min(self.jump_cache_count - 1);

        evaluator.cursor = 0;

        if self.frames.len() < 0x10001 {
            // 16-bit frame keys
            let jump_cache_size = 24 * self.joints.len();
            let cache_start = jump_cache_id * jump_cache_size;

            for joint_id in 0..self.joints.len() {
                let offset = cache_start + joint_id * 24;
                if offset + 24 > self.jump_caches.len() {
                    continue;
                }

                // Read JumpFrameU16 (12 u16 values = 24 bytes)
                let bytes = &self.jump_caches[offset..offset + 24];
                let jump_frame = JumpFrameU16 {
                    rotation_keys: [
                        u16::from_le_bytes([bytes[0], bytes[1]]),
                        u16::from_le_bytes([bytes[2], bytes[3]]),
                        u16::from_le_bytes([bytes[4], bytes[5]]),
                        u16::from_le_bytes([bytes[6], bytes[7]]),
                    ],
                    translation_keys: [
                        u16::from_le_bytes([bytes[8], bytes[9]]),
                        u16::from_le_bytes([bytes[10], bytes[11]]),
                        u16::from_le_bytes([bytes[12], bytes[13]]),
                        u16::from_le_bytes([bytes[14], bytes[15]]),
                    ],
                    scale_keys: [
                        u16::from_le_bytes([bytes[16], bytes[17]]),
                        u16::from_le_bytes([bytes[18], bytes[19]]),
                        u16::from_le_bytes([bytes[20], bytes[21]]),
                        u16::from_le_bytes([bytes[22], bytes[23]]),
                    ],
                };

                self.initialize_joint_hot_frame_u16(evaluator, joint_id, &jump_frame);
            }
        } else {
            // 32-bit frame keys
            let jump_cache_size = 48 * self.joints.len();
            let cache_start = jump_cache_id * jump_cache_size;

            for joint_id in 0..self.joints.len() {
                let offset = cache_start + joint_id * 48;
                if offset + 48 > self.jump_caches.len() {
                    continue;
                }

                // Read JumpFrameU32 (12 u32 values = 48 bytes)
                let bytes = &self.jump_caches[offset..offset + 48];
                let jump_frame = JumpFrameU32 {
                    rotation_keys: [
                        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
                        u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
                        u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
                        u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
                    ],
                    translation_keys: [
                        u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
                        u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
                        u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
                        u32::from_le_bytes([bytes[28], bytes[29], bytes[30], bytes[31]]),
                    ],
                    scale_keys: [
                        u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]),
                        u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]),
                        u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
                        u32::from_le_bytes([bytes[44], bytes[45], bytes[46], bytes[47]]),
                    ],
                };

                self.initialize_joint_hot_frame_u32(evaluator, joint_id, &jump_frame);
            }
        }

        evaluator.cursor += 1;
    }

    fn initialize_joint_hot_frame_u16(
        &self,
        evaluator: &mut HotFrameEvaluator,
        joint_id: usize,
        jump_frame: &JumpFrameU16,
    ) {
        let mut hot_frame = JointHotFrame::default();

        // Initialize rotation hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.rotation_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                let quat = quantized::decompress_quat(&[
                    frame.value()[0] as u8,
                    (frame.value()[0] >> 8) as u8,
                    frame.value()[1] as u8,
                    (frame.value()[1] >> 8) as u8,
                    frame.value()[2] as u8,
                    (frame.value()[2] >> 8) as u8,
                ]);
                hot_frame.rotation[i] = QuaternionHotFrame {
                    time: frame.time(),
                    value: quat,
                };
            }
        }

        // Initialize translation hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.translation_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                hot_frame.translation[i] = VectorHotFrame {
                    time: frame.time(),
                    value: decompress_vector3(
                        &frame.value(),
                        self.translation_min,
                        self.translation_max,
                    ),
                };
            }
        }

        // Initialize scale hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.scale_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                hot_frame.scale[i] = VectorHotFrame {
                    time: frame.time(),
                    value: decompress_vector3(&frame.value(), self.scale_min, self.scale_max),
                };
            }
        }

        // Rotate quaternions along shortest path
        for i in 1..4 {
            if hot_frame.rotation[i].value.dot(hot_frame.rotation[0].value) < 0.0 {
                hot_frame.rotation[i].value = -hot_frame.rotation[i].value;
            }
        }

        evaluator.hot_frames[joint_id] = hot_frame;
    }

    fn initialize_joint_hot_frame_u32(
        &self,
        evaluator: &mut HotFrameEvaluator,
        joint_id: usize,
        jump_frame: &JumpFrameU32,
    ) {
        let mut hot_frame = JointHotFrame::default();

        // Initialize rotation hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.rotation_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                let quat = quantized::decompress_quat(&[
                    frame.value()[0] as u8,
                    (frame.value()[0] >> 8) as u8,
                    frame.value()[1] as u8,
                    (frame.value()[1] >> 8) as u8,
                    frame.value()[2] as u8,
                    (frame.value()[2] >> 8) as u8,
                ]);
                hot_frame.rotation[i] = QuaternionHotFrame {
                    time: frame.time(),
                    value: quat,
                };
            }
        }

        // Initialize translation hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.translation_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                hot_frame.translation[i] = VectorHotFrame {
                    time: frame.time(),
                    value: decompress_vector3(
                        &frame.value(),
                        self.translation_min,
                        self.translation_max,
                    ),
                };
            }
        }

        // Initialize scale hot frames
        for i in 0..4 {
            let frame_idx = jump_frame.scale_keys[i] as usize;
            evaluator.cursor = evaluator.cursor.max(frame_idx);
            if let Some(frame) = self.frames.get(frame_idx) {
                hot_frame.scale[i] = VectorHotFrame {
                    time: frame.time(),
                    value: decompress_vector3(&frame.value(), self.scale_min, self.scale_max),
                };
            }
        }

        // Rotate quaternions along shortest path
        for i in 1..3 {
            if hot_frame.rotation[i].value.dot(hot_frame.rotation[0].value) < 0.0 {
                hot_frame.rotation[i].value = -hot_frame.rotation[i].value;
            }
        }

        evaluator.hot_frames[joint_id] = hot_frame;
    }
}

impl Animation for Compressed {
    fn duration(&self) -> f32 {
        self.duration
    }

    fn fps(&self) -> f32 {
        self.fps
    }

    fn joint_count(&self) -> usize {
        self.joints.len()
    }

    fn joints(&self) -> Cow<'_, [u32]> {
        Cow::Borrowed(&self.joints)
    }

    fn evaluate(&self, time: f32) -> HashMap<u32, (Quat, Vec3, Vec3)> {
        Compressed::evaluate(self, time)
    }
}

impl From<Compressed> for AnimationAsset {
    fn from(val: Compressed) -> Self {
        AnimationAsset::Compressed(val)
    }
}
