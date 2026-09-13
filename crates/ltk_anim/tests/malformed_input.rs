//! Malformed input handling for animation readers

use ltk_anim::{AssetParseError, Compressed, ParseError, RigResource};
use std::io::Cursor;

const COMPRESSED_HEADER_LEN: usize = 128;

/// Builds a compressed animation with one frame per entry. The frame carries
/// `raw_joint_id` in full, including the transform type bits.
fn build_compressed(
    joint_count: u32,
    frame_count: u32,
    jump_cache_count: i32,
    duration: f32,
    raw_joint_id: u16,
) -> Vec<u8> {
    let joints_abs = COMPRESSED_HEADER_LEN;
    let frames_abs = joints_abs + joint_count as usize * 4;
    let caches_abs = frames_abs + frame_count as usize * 10;
    let jump_frame_size = if frame_count < 0x10001 { 24 } else { 48 };
    let caches_len = jump_cache_count.max(0) as usize * jump_frame_size * joint_count as usize;

    let mut buf = Vec::new();
    buf.extend_from_slice(b"r3d2canm");
    buf.extend_from_slice(&1u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u32.to_le_bytes()); // resource size
    buf.extend_from_slice(&0u32.to_le_bytes()); // format token
    buf.extend_from_slice(&0u32.to_le_bytes()); // flags
    buf.extend_from_slice(&joint_count.to_le_bytes());
    buf.extend_from_slice(&frame_count.to_le_bytes());
    buf.extend_from_slice(&jump_cache_count.to_le_bytes());
    buf.extend_from_slice(&duration.to_le_bytes());
    buf.extend_from_slice(&60.0f32.to_le_bytes()); // fps
    for _ in 0..6 {
        buf.extend_from_slice(&0.0f32.to_le_bytes()); // error metrics
    }
    for _ in 0..12 {
        buf.extend_from_slice(&0.0f32.to_le_bytes()); // translation/scale bounds
    }
    buf.extend_from_slice(&((frames_abs - 12) as i32).to_le_bytes());
    buf.extend_from_slice(&((caches_abs - 12) as i32).to_le_bytes());
    buf.extend_from_slice(&((joints_abs - 12) as i32).to_le_bytes());
    assert_eq!(buf.len(), COMPRESSED_HEADER_LEN);

    for joint in 0..joint_count {
        buf.extend_from_slice(&joint.to_le_bytes());
    }
    for _ in 0..frame_count {
        buf.extend_from_slice(&0u16.to_le_bytes()); // time
        buf.extend_from_slice(&raw_joint_id.to_le_bytes());
        buf.extend_from_slice(&[0u8; 6]); // value
    }
    buf.resize(buf.len() + caches_len, 0);

    buf
}

#[test]
fn rig_reader_rejects_unknown_format_token() {
    let result = RigResource::from_reader(&mut Cursor::new([0u8; 8]));
    assert!(matches!(result, Err(ParseError::InvalidFileSignature)));
}

#[test]
fn compressed_reader_rejects_negative_jump_cache_count() {
    let mut buf = build_compressed(1, 1, 0, 1.0, 0);
    buf[32..36].copy_from_slice(&(-1i32).to_le_bytes());

    let result = Compressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("jump cache count", _))
    ));
}

#[test]
fn compressed_reader_rejects_non_finite_duration() {
    let mut buf = build_compressed(1, 1, 0, 1.0, 0);
    buf[36..40].copy_from_slice(&f32::NAN.to_le_bytes());

    let result = Compressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("duration", _))
    ));
}

#[test]
fn compressed_reader_rejects_negative_duration() {
    let mut buf = build_compressed(1, 1, 0, 1.0, 0);
    buf[36..40].copy_from_slice(&(-1.0f32).to_le_bytes());

    let result = Compressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("duration", _))
    ));
}

#[test]
fn compressed_reader_rejects_frame_joint_id_out_of_range() {
    let buf = build_compressed(1, 1, 0, 1.0, 5);

    let result = Compressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame joint id", _))
    ));
}

#[test]
fn compressed_reader_rejects_frame_transform_type() {
    let buf = build_compressed(1, 1, 0, 1.0, 0xC000);

    let result = Compressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame transform type", _))
    ));
}

#[test]
fn uncompressed_reader_rejects_non_finite_frame_duration() {
    let mut buf = Vec::new();
    buf.extend_from_slice(b"r3d2anmd");
    buf.extend_from_slice(&5u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u32.to_le_bytes()); // resource size
    buf.extend_from_slice(&0u32.to_le_bytes()); // format token
    buf.extend_from_slice(&5u32.to_le_bytes()); // version
    buf.extend_from_slice(&0u32.to_le_bytes()); // flags
    buf.extend_from_slice(&1u32.to_le_bytes()); // track count
    buf.extend_from_slice(&1u32.to_le_bytes()); // frame count
    buf.extend_from_slice(&f32::NAN.to_le_bytes()); // frame duration

    let result = ltk_anim::Uncompressed::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame duration", _))
    ));
}
