//! Rotations read from a stream.

use glam::Quat;

/// Scales `rotation` to unit length with [`Quat::normalize`].
///
/// Returns `None` for a rotation whose length is zero or not finite, and for one whose
/// normalized form is not within glam's unit length tolerance. glam asserts that a rotation
/// is normalized, and the workspace enables those assertions.
pub(crate) fn try_normalize(rotation: Quat) -> Option<Quat> {
    let length = rotation.length();
    if length.is_finite() && length > 0.0 {
        Some(rotation.normalize()).filter(|normalized| normalized.is_normalized())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_normalize_matches_normalize_bit_for_bit() {
        let rotation = Quat::from_xyzw(0.1, -0.7, 0.3, 0.64);

        let normalized = try_normalize(rotation).unwrap();

        assert_eq!(normalized.to_array(), rotation.normalize().to_array());
    }

    #[test]
    fn try_normalize_rejects_zero_and_non_finite_rotations() {
        assert_eq!(try_normalize(Quat::from_xyzw(0.0, 0.0, 0.0, 0.0)), None);
        assert_eq!(
            try_normalize(Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0)),
            None
        );
        assert_eq!(
            try_normalize(Quat::from_xyzw(f32::MAX, f32::MAX, 0.0, 0.0)),
            None
        );
    }
}
