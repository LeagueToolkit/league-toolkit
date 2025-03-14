use std::f32::consts::{FRAC_1_SQRT_2, SQRT_2};

use glam::Quat;
use itertools::Itertools;

#[repr(transparent)]
pub struct CompressedQuat(pub [u16; 3]);

impl CompressedQuat {
    pub fn compress(mut quat: Quat) -> Self {
        let omit_idx;
        let x_abs = quat.x.abs();
        let y_abs = quat.y.abs();
        let z_abs = quat.z.abs();
        let w_abs = quat.w.abs();
        if x_abs >= w_abs && x_abs >= y_abs && x_abs >= z_abs {
            omit_idx = 0;
            if quat.x < 0.0 {
                quat = -quat;
            }
        } else if y_abs >= w_abs && y_abs >= x_abs && y_abs >= z_abs {
            omit_idx = 1;
            if quat.y < 0.0 {
                quat = -quat;
            }
        } else if z_abs >= w_abs && z_abs >= x_abs && z_abs >= y_abs {
            omit_idx = 2;
            if quat.z < 0.0 {
                quat = -quat;
            }
        } else {
            omit_idx = 3;
            if quat.w < 0.0 {
                quat = -quat;
            }
        }

        let quat = quat.to_array();

        let mut bits = (omit_idx as u64) << 45;
        let mut component_off = 2;
        for (i, component) in quat.into_iter().enumerate() {
            if i == omit_idx {
                continue;
            }

            let component = (32767.0 / 2.0 * (SQRT_2 * component + 1.0)).round() as u16;
            bits |= ((component as u64) & ((1 << 15) - 1)) << (15 * component_off);
            component_off -= 1;
        }

        // Safety: The range is 0..3, so collect_array can always collect exactly 3 elements
        CompressedQuat(unsafe {
            (0..3)
                .map(|i| (bits >> (16 * i) & ((1 << 16) - 1)) as u16)
                .collect_array()
                .unwrap_unchecked()
        })
    }

    pub fn decompress(self) -> Quat {
        let bits = u64::from(self.0[0]) | u64::from(self.0[1]) << 16 | u64::from(self.0[2]) << 32;
        let max_index = bits >> 45 & ((1 << 2) - 1);

        let mask = (1 << 15) - 1;
        let v_a = bits >> 30 & mask;
        let v_b = bits >> 15 & mask;
        let v_c = bits & mask;

        let a = (v_a as f32 / 32767.0) * SQRT_2 - FRAC_1_SQRT_2;
        let b = (v_b as f32 / 32767.0) * SQRT_2 - FRAC_1_SQRT_2;
        let c = (v_c as f32 / 32767.0) * SQRT_2 - FRAC_1_SQRT_2;
        let sub = (1.0 - (a * a + b * b + c * c)).max(0.0);
        let d = sub.sqrt();

        match max_index {
            0 => Quat::from_array([d, a, b, c]),
            1 => Quat::from_array([a, d, b, c]),
            2 => Quat::from_array([a, b, d, c]),
            3 => Quat::from_array([a, b, c, d]),
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let n = Quat::from_xyzw(123.0, 3021.0, 65904.0, 33.0).normalize();
        let comp = CompressedQuat::compress(n);
        let n = n.to_array();
        let decomp = comp.decompress().to_array();

        for (c, (a, b)) in n.iter().zip(decomp.iter()).enumerate() {
            assert!(
                (b - a).abs() < 0.05,
                "delta of component {c} >= 0.05 ({a} vs {b})"
            )
        }
    }
}
