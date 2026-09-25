//! Evaluation of compressed animations that start from their jump caches

mod common;

use common::{push_jump_frame_u16, CompressedClip, RawFrame, Transform};
use glam::{Quat, Vec3};
use ltk_anim::Compressed;
use std::io::Cursor;

const JOINT: u32 = 0xCAFE_0001;

/// Builds a one-joint clip whose four keys per transform all hold the same pose.
///
/// The keys sit at times 0, 1/3, 2/3 and 1. The one jump cache points at all
/// twelve frames. Every key agrees, so every sample time evaluates to that pose.
fn constant_clip(rotation: Quat, translation: [u16; 3], scale: [u16; 3]) -> CompressedClip {
    let times = [0, 21845, 43690, 65535];
    let mut frames = Vec::new();
    for time in times {
        frames.push(RawFrame::rotation(time, 0, rotation));
        frames.push(RawFrame::new(time, 0, Transform::Translation, translation));
        frames.push(RawFrame::new(time, 0, Transform::Scale, scale));
    }

    let mut jump_caches = Vec::new();
    push_jump_frame_u16(&mut jump_caches, [0, 3, 6, 9], [1, 4, 7, 10], [2, 5, 8, 11]);

    CompressedClip {
        // A quantized value maps onto itself between these bounds.
        translation_min: [0.0; 3],
        translation_max: [65535.0; 3],
        scale_min: [0.0; 3],
        scale_max: [65535.0; 3],
        joints: vec![JOINT],
        frames,
        jump_cache_count: 1,
        jump_caches,
        ..CompressedClip::default()
    }
}

#[test]
fn evaluate_starts_from_the_jump_cache_pose() {
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let clip = constant_clip(rotation, [1, 2, 3], [4, 5, 6]).to_bytes();
    let animation = Compressed::from_reader(&mut Cursor::new(clip)).unwrap();

    for time in [0.0, 0.25, 0.5, 0.75] {
        let pose = animation.evaluate(time);
        let (r, t, s) = pose[&JOINT];

        assert!(
            r.angle_between(rotation) < 0.01,
            "rotation at t = {time} is {r}, expected {rotation}"
        );
        let (translation, scale) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0));
        assert!(
            t.abs_diff_eq(translation, 1e-3),
            "translation at t = {time} is {t}, expected {translation}"
        );
        assert!(
            s.abs_diff_eq(scale, 1e-3),
            "scale at t = {time} is {s}, expected {scale}"
        );
    }
}

/// Shipped files set exporter bits `0x8`, `0x10` and `0x20`, which the reader used to reject.
#[test]
fn exporter_flags_do_not_stop_parsing() {
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let clip = CompressedClip {
        flags: 0x3F,
        ..constant_clip(rotation, [1, 2, 3], [4, 5, 6])
    };
    let animation = Compressed::from_reader(&mut Cursor::new(clip.to_bytes())).unwrap();

    let (r, _, _) = animation.evaluate(0.5)[&JOINT];
    assert!(
        r.angle_between(rotation) < 0.01,
        "rotation is {r}, expected {rotation}"
    );
}

/// The flags bit that selects the key time weighted spline.
const USE_KEYFRAME_PARAMETRIZATION: u32 = 0x4;

/// A jump cache index that marks a transform without keys.
const NO_KEYS: u16 = 0xFFFF;

/// Pruned files store a transform that never changes as one key, and leave an unused
/// transform out. One key used to collapse the weighted spline to zero and panic in
/// `Quat::normalize`. A transform without keys used to sample a zero scale.
#[test]
fn a_single_key_or_no_key_samples_the_key_or_identity() {
    let rotation = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let mut jump_caches = Vec::new();
    push_jump_frame_u16(&mut jump_caches, [0; 4], [1; 4], [NO_KEYS; 4]);
    let clip = CompressedClip {
        flags: USE_KEYFRAME_PARAMETRIZATION,
        translation_min: [0.0; 3],
        translation_max: [65535.0; 3],
        joints: vec![JOINT],
        frames: vec![
            RawFrame::rotation(0, 0, rotation),
            RawFrame::new(0, 0, Transform::Translation, [1, 2, 3]),
        ],
        jump_cache_count: 1,
        jump_caches,
        ..CompressedClip::default()
    };
    let animation = Compressed::from_reader(&mut Cursor::new(clip.to_bytes())).unwrap();

    let (r, t, s) = animation.evaluate(0.5)[&JOINT];
    assert!(
        r.angle_between(rotation) < 0.01,
        "rotation is {r}, expected {rotation}"
    );
    let translation = Vec3::new(1.0, 2.0, 3.0);
    assert!(
        t.abs_diff_eq(translation, 1e-3),
        "translation is {t}, expected {translation}"
    );
    assert_eq!(s, Vec3::ONE);
}

/// The `0xFFFF` of a transform without keys must not move the frame cursor. It used to
/// move it past every frame, so no key after the jump cache was ever read.
#[test]
fn a_transform_without_keys_does_not_stop_later_keys() {
    let (a, b) = (
        Quat::IDENTITY,
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
    );
    let mut jump_caches = Vec::new();
    push_jump_frame_u16(&mut jump_caches, [0, 1, 2, 3], [4; 4], [NO_KEYS; 4]);
    let clip = CompressedClip {
        flags: USE_KEYFRAME_PARAMETRIZATION,
        joints: vec![JOINT],
        frames: vec![
            RawFrame::rotation(0, 0, a),
            RawFrame::rotation(16384, 0, a),
            RawFrame::rotation(32768, 0, a),
            RawFrame::rotation(49151, 0, b),
            RawFrame::new(0, 0, Transform::Translation, [0; 3]),
            // Only reachable by advancing the cursor past the jump cache.
            RawFrame::rotation(65535, 0, b),
        ],
        jump_cache_count: 1,
        jump_caches,
        ..CompressedClip::default()
    };
    let animation = Compressed::from_reader(&mut Cursor::new(clip.to_bytes())).unwrap();

    // 0.75 compresses to 49151, the time of the first `b` key.
    let (r, _, _) = animation.evaluate(0.75)[&JOINT];
    assert!(r.angle_between(b) < 0.01, "rotation is {r}, expected {b}");
}
