//! Builds animation files byte by byte.

#![expect(dead_code, reason = "each test binary uses a different subset")]

use glam::Quat;
use ltk_anim::quantized;

/// Byte offsets of the fields of a compressed (`r3d2canm`) header.
pub mod compressed_header {
    pub const JOINT_COUNT: usize = 24;
    pub const FRAME_COUNT: usize = 28;
    pub const JUMP_CACHE_COUNT: usize = 32;
    pub const DURATION: usize = 36;
    pub const LEN: usize = 128;
}

/// Byte offsets of the fields of an uncompressed (`r3d2anmd`) v4 or v5 header.
pub mod uncompressed_header {
    pub const TRACK_COUNT: usize = 28;
    pub const FRAME_COUNT: usize = 32;
    pub const FRAME_DURATION: usize = 36;
    pub const JOINT_HASHES_OFFSET: usize = 40;
    pub const VECTOR_PALETTE_OFFSET: usize = 52;
    pub const QUAT_PALETTE_OFFSET: usize = 56;
    pub const FRAMES_OFFSET: usize = 60;
    pub const LEN: usize = 64;
}

/// Overwrites the four bytes at `at` with `bytes`.
pub fn patch(buf: &mut [u8], at: usize, bytes: [u8; 4]) {
    buf[at..at + 4].copy_from_slice(&bytes);
}

/// Reads the little-endian `i32` at `at`.
pub fn read_i32(buf: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// The transform a compressed frame keys, stored in the top two bits of its joint id.
#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Rotation = 0,
    Translation = 1,
    Scale = 2,
}

/// One compressed frame as stored in the file.
#[derive(Clone, Copy, Debug)]
pub struct RawFrame {
    pub time: u16,
    /// The joint index in the low 14 bits, the transform type in the top 2.
    pub joint_id: u16,
    pub value: [u16; 3],
}

impl RawFrame {
    pub fn new(time: u16, joint: u16, transform: Transform, value: [u16; 3]) -> Self {
        Self {
            time,
            joint_id: joint | ((transform as u16) << 14),
            value,
        }
    }

    /// A rotation key holding `rotation` quantized to 48 bits.
    pub fn rotation(time: u16, joint: u16, rotation: Quat) -> Self {
        let bytes = quantized::compress_quat(rotation);
        let value = [
            u16::from_le_bytes([bytes[0], bytes[1]]),
            u16::from_le_bytes([bytes[2], bytes[3]]),
            u16::from_le_bytes([bytes[4], bytes[5]]),
        ];
        Self::new(time, joint, Transform::Rotation, value)
    }
}

/// A compressed (`r3d2canm`) animation.
///
/// The file holds the header, then the joint hashes, the frames and the jump caches.
#[derive(Clone, Debug)]
pub struct CompressedClip {
    pub flags: u32,
    pub duration: f32,
    pub fps: f32,
    pub translation_min: [f32; 3],
    pub translation_max: [f32; 3],
    pub scale_min: [f32; 3],
    pub scale_max: [f32; 3],
    pub joints: Vec<u32>,
    pub frames: Vec<RawFrame>,
    pub jump_cache_count: i32,
    /// The jump cache bytes: `jump_cache_count` caches of one jump frame per joint.
    pub jump_caches: Vec<u8>,
}

impl Default for CompressedClip {
    fn default() -> Self {
        Self {
            flags: 0,
            duration: 1.0,
            fps: 30.0,
            translation_min: [0.0; 3],
            translation_max: [1.0; 3],
            scale_min: [0.0; 3],
            scale_max: [1.0; 3],
            joints: vec![0x1234_5678],
            frames: vec![RawFrame::rotation(0, 0, Quat::IDENTITY)],
            jump_cache_count: 0,
            jump_caches: Vec::new(),
        }
    }
}

impl CompressedClip {
    pub fn to_bytes(&self) -> Vec<u8> {
        let joints_at = compressed_header::LEN;
        let frames_at = joints_at + self.joints.len() * 4;
        let jump_caches_at = frames_at + self.frames.len() * 10;

        let mut buf = Vec::new();
        buf.extend_from_slice(b"r3d2canm");
        buf.extend_from_slice(&1u32.to_le_bytes()); // version
        buf.extend_from_slice(&0u32.to_le_bytes()); // resource size
        buf.extend_from_slice(&0u32.to_le_bytes()); // format token
        buf.extend_from_slice(&self.flags.to_le_bytes());
        buf.extend_from_slice(&(self.joints.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(self.frames.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.jump_cache_count.to_le_bytes());
        buf.extend_from_slice(&self.duration.to_le_bytes());
        buf.extend_from_slice(&self.fps.to_le_bytes());
        for _ in 0..3 {
            buf.extend_from_slice(&2.0f32.to_le_bytes()); // error metric margin
            buf.extend_from_slice(&10.0f32.to_le_bytes()); // discontinuity threshold
        }
        for bound in [
            self.translation_min,
            self.translation_max,
            self.scale_min,
            self.scale_max,
        ] {
            for component in bound {
                buf.extend_from_slice(&component.to_le_bytes());
            }
        }
        // Section offsets are stored relative to byte 12.
        buf.extend_from_slice(&((frames_at - 12) as i32).to_le_bytes());
        buf.extend_from_slice(&((jump_caches_at - 12) as i32).to_le_bytes());
        buf.extend_from_slice(&((joints_at - 12) as i32).to_le_bytes());
        assert_eq!(buf.len(), compressed_header::LEN);

        for joint in &self.joints {
            buf.extend_from_slice(&joint.to_le_bytes());
        }
        for frame in &self.frames {
            buf.extend_from_slice(&frame.time.to_le_bytes());
            buf.extend_from_slice(&frame.joint_id.to_le_bytes());
            for component in frame.value {
                buf.extend_from_slice(&component.to_le_bytes());
            }
        }
        buf.extend_from_slice(&self.jump_caches);
        buf
    }
}

/// Appends one 16-bit jump frame: the frame indices of four keys per transform.
pub fn push_jump_frame_u16(
    jump_caches: &mut Vec<u8>,
    rotation: [u16; 4],
    translation: [u16; 4],
    scale: [u16; 4],
) {
    for key in rotation.into_iter().chain(translation).chain(scale) {
        jump_caches.extend_from_slice(&key.to_le_bytes());
    }
}

/// One uncompressed v4 track key: a joint hash and palette indices.
#[derive(Clone, Copy, Debug)]
pub struct V4Key {
    pub joint_hash: u32,
    pub translation_id: u16,
    pub scale_id: u16,
    pub rotation_id: u16,
}

/// An uncompressed v4 (`r3d2anmd`) animation.
///
/// The file holds the header, then the vector palette, the quaternion palette and the frames.
/// `frames` holds `track_count` keys per frame.
#[derive(Clone, Debug)]
pub struct V4Clip {
    pub track_count: u32,
    pub frame_duration: f32,
    pub vectors: Vec<[f32; 3]>,
    /// Quaternions in `x, y, z, w` order.
    pub quats: Vec<[f32; 4]>,
    pub frames: Vec<Vec<V4Key>>,
}

impl Default for V4Clip {
    fn default() -> Self {
        let key = |joint_hash| V4Key {
            joint_hash,
            translation_id: 0,
            scale_id: 1,
            rotation_id: 0,
        };
        Self {
            track_count: 2,
            frame_duration: 1.0 / 30.0,
            vectors: vec![[0.0; 3], [1.0; 3]],
            quats: vec![[0.0, 0.0, 0.0, 1.0]],
            frames: vec![vec![key(0xA), key(0xB)], vec![key(0xA), key(0xB)]],
        }
    }
}

impl V4Clip {
    pub fn to_bytes(&self) -> Vec<u8> {
        let vectors_at = uncompressed_header::LEN;
        let quats_at = vectors_at + self.vectors.len() * 12;
        let frames_at = quats_at + self.quats.len() * 16;

        let mut buf = Vec::new();
        buf.extend_from_slice(b"r3d2anmd");
        buf.extend_from_slice(&4u32.to_le_bytes()); // version
        buf.extend_from_slice(&0u32.to_le_bytes()); // resource size
        buf.extend_from_slice(&0u32.to_le_bytes()); // format token
        buf.extend_from_slice(&4u32.to_le_bytes()); // version
        buf.extend_from_slice(&0u32.to_le_bytes()); // flags
        buf.extend_from_slice(&self.track_count.to_le_bytes());
        buf.extend_from_slice(&(self.frames.len() as u32).to_le_bytes());
        buf.extend_from_slice(&self.frame_duration.to_le_bytes());
        // Section offsets are stored relative to byte 12.
        for offset in [0, 0, 0, vectors_at - 12, quats_at - 12, frames_at - 12] {
            buf.extend_from_slice(&(offset as i32).to_le_bytes());
        }
        assert_eq!(buf.len(), uncompressed_header::LEN);

        for vector in &self.vectors {
            for component in vector {
                buf.extend_from_slice(&component.to_le_bytes());
            }
        }
        for quat in &self.quats {
            for component in quat {
                buf.extend_from_slice(&component.to_le_bytes());
            }
        }
        for frame in &self.frames {
            for key in frame {
                buf.extend_from_slice(&key.joint_hash.to_le_bytes());
                buf.extend_from_slice(&key.translation_id.to_le_bytes());
                buf.extend_from_slice(&key.scale_id.to_le_bytes());
                buf.extend_from_slice(&key.rotation_id.to_le_bytes());
                buf.extend_from_slice(&0u16.to_le_bytes()); // padding
            }
        }
        buf
    }
}

/// Byte offsets of the fields of an uncompressed (`r3d2anmd`) v3 header.
pub mod v3_header {
    pub const TRACK_COUNT: usize = 16;
    pub const FRAME_COUNT: usize = 20;
}

/// An uncompressed v3 (`r3d2anmd`) animation of identity keys.
///
/// The file holds the header, then each track: a 32-byte name, 4 bytes of flags and a
/// 28-byte key per frame.
pub fn v3_clip(track_count: u32, frame_count: u32) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"r3d2anmd");
    buf.extend_from_slice(&3u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u32.to_le_bytes()); // skeleton id
    buf.extend_from_slice(&track_count.to_le_bytes());
    buf.extend_from_slice(&frame_count.to_le_bytes());
    buf.extend_from_slice(&30u32.to_le_bytes()); // fps
    for track in 0..track_count {
        let mut name = [0u8; 32];
        let label = format!("joint{track}");
        name[..label.len()].copy_from_slice(label.as_bytes());
        buf.extend_from_slice(&name);
        buf.extend_from_slice(&0u32.to_le_bytes()); // flags
        for _ in 0..frame_count {
            for component in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0] {
                buf.extend_from_slice(&component.to_le_bytes());
            }
        }
    }
    buf
}
