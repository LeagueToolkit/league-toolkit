use super::Sphere;
use glam::{vec3, Vec3};

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
}

fn dist(a: &Vec3, b: &Vec3) -> f32 {
    ((a[0] - b[0]).powf(2.0) + (a[1] - b[1]).powf(2.0) + (a[2] - b[2]).powf(2.0)).sqrt()
}

impl AABB {
    pub fn center(&self) -> Vec3 {
        Vec3::new(
            0.5 * (self.min[0] + self.max[0]),
            0.5 * (self.min[1] + self.max[1]),
            0.5 * (self.min[2] + self.max[2]),
        )
    }
    pub fn bounding_sphere(&self) -> Sphere {
        let center = self.center();
        Sphere::new(center, dist(&center, &self.max))
    }

    pub fn from_vertex_iter(verts: impl IntoIterator<Item = Vec3>) -> Self {
        let mut min = vec3(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let mut max = vec3(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
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
