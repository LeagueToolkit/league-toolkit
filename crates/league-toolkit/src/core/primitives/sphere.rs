use vecmath::Vector3;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Sphere {
    origin: Vector3<f32>,
    radius: f32,
}

impl Sphere {
    pub const INFINITE: Sphere = Self::new([0.0, 0.0, 0.0], f32::INFINITY);

    pub const fn new(origin: Vector3<f32>, radius: f32) -> Self {
        Self { origin, radius }
    }
}
