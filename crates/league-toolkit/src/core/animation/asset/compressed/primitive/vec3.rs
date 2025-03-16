use glam::Vec3;

pub struct CompressedVec3(pub [u16; 3]);

impl CompressedVec3 {
    pub fn new(compressed: [u16; 3]) -> Self {
        Self(compressed)
    }
    pub fn compress(value: Vec3, min: Vec3, max: Vec3) -> Self {
        let scaled = (u16::MAX as f32 * (value - min)) / (max - min);
        Self([scaled.x as u16, scaled.y as u16, scaled.z as u16])
    }

    pub fn decompress(self, min: Vec3, max: Vec3) -> Vec3 {
        let mut val = max - min;
        let scale = u16::MAX as f32;

        val.x *= (self.0[0] as f32) / scale;
        val.y *= (self.0[1] as f32) / scale;
        val.z *= (self.0[2] as f32) / scale;

        val += min;
        val
    }
}
#[cfg(test)]
mod tests {
    use glam::vec3;

    use super::*;
    #[test]
    fn roundtrip() {
        let n = vec3(5.1, 33.3333, 23.1234);
        let (min, max) = (vec3(5.0, 10.0, 3.0), vec3(100.0, 50.0, 38.5));
        let comp = CompressedVec3::compress(n, min, max);
        let n = n.to_array();
        let decomp = comp.decompress(min, max).to_array();

        for (c, (a, b)) in n.iter().zip(decomp.iter()).enumerate() {
            assert!(
                (b - a).abs() < 0.05,
                "delta of component {c} >= 0.05\n      got: {decomp:?}\n expected: {n:?}"
            )
        }
    }
}
