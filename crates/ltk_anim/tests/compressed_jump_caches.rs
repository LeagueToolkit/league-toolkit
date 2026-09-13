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
