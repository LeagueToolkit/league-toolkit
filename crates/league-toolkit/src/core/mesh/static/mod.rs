use glam::Vec3;

pub use face::*;

use crate::core::primitives::Color;

mod face;
mod read;

const MAGIC: &[u8] = "r3d2Mesh".as_bytes();

#[derive(Clone, Debug)]
pub struct StaticMesh {
    name: String,

    vertices: Vec<Vec3>,
    faces: Vec<StaticMeshFace>,
    vertex_colors: Option<Vec<Color>>,
}

// TODO (alan): figure out endianness

impl StaticMesh {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn vertices(&self) -> &[Vec3] {
        &self.vertices
    }

    pub fn faces(&self) -> &[StaticMeshFace] {
        &self.faces
    }

    pub fn vertex_colors(&self) -> Option<&Vec<Color>> {
        self.vertex_colors.as_ref()
    }
}
