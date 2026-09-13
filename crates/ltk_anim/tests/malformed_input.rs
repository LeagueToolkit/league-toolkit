//! Malformed input handling for animation readers

mod common;

use common::{
    compressed_header, patch, read_i32, uncompressed_header, v3_clip, v3_header, CompressedClip,
    RawFrame, Transform, V4Clip, V4Key,
};
use glam::Quat;
use ltk_anim::{
    AnimationAsset, AssetParseError, Compressed, JointBuilder, ParseError, RigResource,
    Uncompressed,
};
use std::collections::HashMap;
use std::io::{self, Cursor};

fn read_compressed(buf: Vec<u8>) -> ltk_anim::asset::Result<Compressed> {
    Compressed::from_reader(&mut Cursor::new(buf))
}

fn read_uncompressed(buf: Vec<u8>) -> ltk_anim::asset::Result<Uncompressed> {
    Uncompressed::from_reader(&mut Cursor::new(buf))
}

fn read_rig(buf: Vec<u8>) -> ltk_anim::Result<RigResource> {
    RigResource::from_reader(&mut Cursor::new(buf))
}

/// A v5 clip of two joints and two frames, as the writer lays it out.
fn v5_clip() -> Vec<u8> {
    let frames = vec![ltk_anim::asset::UncompressedFrame::default(); 2];
    let animation = Uncompressed::new(
        30.0,
        vec![glam::Vec3::ZERO],
        vec![Quat::IDENTITY],
        HashMap::from([(0xA, frames.clone()), (0xB, frames)]),
    );
    let mut buf = Cursor::new(Vec::new());
    animation.to_writer(&mut buf).unwrap();
    buf.into_inner()
}

/// A skeleton of two joints, one of them an influence, as the writer lays it out.
fn rig() -> Vec<u8> {
    let rig = RigResource::builder("rig", "asset")
        .with_root_joint(
            JointBuilder::new("root")
                .with_children([JointBuilder::new("child").with_influence(true)]),
        )
        .build();
    let mut buf = Cursor::new(Vec::new());
    rig.to_writer(&mut buf).unwrap();
    buf.into_inner()
}

#[test]
fn well_formed_inputs_read() {
    read_compressed(CompressedClip::default().to_bytes()).unwrap();
    read_uncompressed(v5_clip()).unwrap();
    read_uncompressed(V4Clip::default().to_bytes()).unwrap();
    read_uncompressed(v3_clip(2, 3)).unwrap();
    read_rig(rig()).unwrap();
}

#[test]
fn rig_reader_rejects_unknown_format_token() {
    let result = RigResource::from_reader(&mut Cursor::new([0u8; 8]));
    assert!(matches!(result, Err(ParseError::InvalidFileSignature)));
}

#[test]
fn rig_reader_rejects_influence_count_past_the_stream_end() {
    let mut buf = rig();
    patch(&mut buf, 16, u32::MAX.to_le_bytes());

    let result = read_rig(buf);
    assert!(matches!(
        result,
        Err(ParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn rig_reader_rejects_joint_rotation_with_no_unit_length() {
    let mut buf = rig();
    // The first joint starts at the joints offset. Its local rotation is at bytes 40..56.
    let joint = usize::try_from(read_i32(&buf, 20)).unwrap();
    buf[joint + 40..joint + 56].fill(0);

    let result = read_rig(buf);
    assert!(matches!(
        result,
        Err(ParseError::ReaderError(error)) if error.kind() == io::ErrorKind::InvalidData
    ));
}

#[test]
fn compressed_reader_rejects_negative_jump_cache_count() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::JUMP_CACHE_COUNT,
        (-1i32).to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("jump cache count", _))
    ));
}

#[test]
fn compressed_reader_rejects_non_finite_duration() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::DURATION,
        f32::NAN.to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("duration", _))
    ));
}

#[test]
fn compressed_reader_rejects_negative_duration() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::DURATION,
        (-1.0f32).to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("duration", _))
    ));
}

#[test]
fn compressed_reader_rejects_frame_joint_id_out_of_range() {
    let clip = CompressedClip {
        frames: vec![RawFrame::new(0, 5, Transform::Rotation, [0; 3])],
        ..CompressedClip::default()
    };

    let result = read_compressed(clip.to_bytes());
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame joint id", _))
    ));
}

#[test]
fn compressed_reader_rejects_frame_transform_type() {
    let clip = CompressedClip {
        frames: vec![RawFrame {
            time: 0,
            joint_id: 0xC000,
            value: [0; 3],
        }],
        ..CompressedClip::default()
    };

    let result = read_compressed(clip.to_bytes());
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame transform type", _))
    ));
}

#[test]
fn compressed_reader_rejects_joint_count_past_the_stream_end() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::JOINT_COUNT,
        u32::MAX.to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn compressed_reader_rejects_frame_count_past_the_stream_end() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::FRAME_COUNT,
        u32::MAX.to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn compressed_reader_rejects_jump_cache_count_past_the_stream_end() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::JUMP_CACHE_COUNT,
        i32::MAX.to_le_bytes(),
    );

    let result = read_compressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn uncompressed_reader_rejects_non_finite_frame_duration() {
    let mut buf = v5_clip();
    patch(
        &mut buf,
        uncompressed_header::FRAME_DURATION,
        f32::NAN.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame duration", _))
    ));
}

#[test]
fn uncompressed_reader_rejects_zero_frame_duration() {
    let mut buf = v5_clip();
    patch(
        &mut buf,
        uncompressed_header::FRAME_DURATION,
        0.0f32.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame duration", _))
    ));
}

#[test]
fn uncompressed_reader_rejects_clip_duration_that_overflows() {
    let mut buf = v5_clip();
    patch(
        &mut buf,
        uncompressed_header::FRAME_DURATION,
        f32::MAX.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("frame duration", _))
    ));
}

#[test]
fn v5_reader_rejects_frame_count_past_the_stream_end() {
    let mut buf = v5_clip();
    patch(
        &mut buf,
        uncompressed_header::FRAME_COUNT,
        u32::MAX.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn v5_reader_rejects_sections_out_of_order() {
    let mut buf = v5_clip();
    // The frames start 4 bytes before the joint hashes. A 4-byte section of negative size
    // is a multiple of its element size once wrapped to an unsigned size.
    let joint_hashes = read_i32(&buf, uncompressed_header::JOINT_HASHES_OFFSET);
    patch(
        &mut buf,
        uncompressed_header::FRAMES_OFFSET,
        (joint_hashes - 4).to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("joint hashes", _))
    ));
}

#[test]
fn v5_reader_rejects_sections_past_the_stream_end() {
    let mut buf = v5_clip();
    for field in [
        uncompressed_header::JOINT_HASHES_OFFSET,
        uncompressed_header::VECTOR_PALETTE_OFFSET,
        uncompressed_header::QUAT_PALETTE_OFFSET,
        uncompressed_header::FRAMES_OFFSET,
    ] {
        let offset = read_i32(&buf, field);
        patch(&mut buf, field, (offset + 1_000_000).to_le_bytes());
    }

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn v5_reader_rejects_more_joint_hashes_than_tracks() {
    let mut buf = v5_clip();
    patch(
        &mut buf,
        uncompressed_header::TRACK_COUNT,
        1u32.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("joint hashes", _))
    ));
}

#[test]
fn v4_reader_rejects_frame_count_past_the_stream_end() {
    let mut buf = V4Clip::default().to_bytes();
    patch(
        &mut buf,
        uncompressed_header::FRAME_COUNT,
        u32::MAX.to_le_bytes(),
    );

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn v4_reader_rejects_more_joints_than_tracks() {
    let key = |joint_hash| V4Key {
        joint_hash,
        translation_id: 0,
        scale_id: 1,
        rotation_id: 0,
    };
    let clip = V4Clip {
        track_count: 1,
        frames: vec![vec![key(0xA)], vec![key(0xB)]],
        ..V4Clip::default()
    };

    let result = read_uncompressed(clip.to_bytes());
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("joint hashes", _))
    ));
}

#[test]
fn v4_reader_rejects_rotation_with_no_unit_length() {
    let clip = V4Clip {
        quats: vec![[0.0; 4]],
        ..V4Clip::default()
    };

    let result = read_uncompressed(clip.to_bytes());
    assert!(matches!(
        result,
        Err(AssetParseError::InvalidField("quaternion palette", _))
    ));
}

#[test]
fn v3_reader_rejects_track_count_past_the_stream_end() {
    let mut buf = v3_clip(2, 3);
    patch(&mut buf, v3_header::TRACK_COUNT, u32::MAX.to_le_bytes());
    patch(&mut buf, v3_header::FRAME_COUNT, u32::MAX.to_le_bytes());

    let result = read_uncompressed(buf);
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}

#[test]
fn animation_asset_reader_rejects_what_the_format_reader_rejects() {
    let mut buf = CompressedClip::default().to_bytes();
    patch(
        &mut buf,
        compressed_header::FRAME_COUNT,
        u32::MAX.to_le_bytes(),
    );

    let result = AnimationAsset::from_reader(&mut Cursor::new(buf));
    assert!(matches!(
        result,
        Err(AssetParseError::ReaderError(error)) if error.kind() == io::ErrorKind::UnexpectedEof
    ));
}
