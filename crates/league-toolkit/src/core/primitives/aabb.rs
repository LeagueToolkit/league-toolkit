use vecmath::Vector3;

use super::Sphere;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct AABB<T> {
    pub min: Vector3<T>,
    pub max: Vector3<T>,
}

impl<T: Default> AABB<T> {
    pub fn new(min: Vector3<T>, max: Vector3<T>) -> Self {
        Self { min, max }
    }
}

impl<T: Default> Default for AABB<T> {
    fn default() -> Self {
        Self {
            min: [T::default(), T::default(), T::default()],
            max: [T::default(), T::default(), T::default()],
        }
    }
}

fn dist(a: &Vector3<f32>, b: &Vector3<f32>) -> f32 {
    ((a[0] - b[0]).powf(2.0) + (a[1] - b[1]).powf(2.0) + (a[2] - b[2]).powf(2.0)).sqrt()
}

impl AABB<f32> {
    pub fn center(&self) -> Vector3<f32> {
        [
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        ]
    }
    pub fn bounding_sphere(&self) -> Sphere {
        let center = self.center();
        Sphere::new(center, dist(&center, &self.max))
    }

    pub fn from_vertex_iter(verts: impl IntoIterator<Item = Vector3<f32>>) -> Self {
        let mut min = [f32::INFINITY, f32::INFINITY, f32::INFINITY];
        let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
        for v in verts {
            for i in 0..3 {
                if v[i] < min[i] {
                    min[i] = v[i];
                }
                if v[i] > max[i] {
                    max[i] = v[i];
                }
            }
        }
        Self { min, max }
    }
}
