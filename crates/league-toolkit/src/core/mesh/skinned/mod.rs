use std::io::Read;

use byteorder::ReadBytesExt;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use vecmath::Vector3;

pub use range::*;

use crate::core::{
    mem::{ElementName, IndexBuffer, VertexBuffer},
    primitives::{AABB, Sphere},
};

use super::Result;

mod range;
mod vertex;
mod read;
mod write;

const MAGIC: u32 = 0x00112233;

#[derive(Debug, PartialEq)]
pub struct SkinnedMesh {
    aabb: AABB<f32>,
    bounding_sphere: Sphere,
    ranges: Vec<SkinnedMeshRange>,
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
}

impl SkinnedMesh {
    pub fn new(
        ranges: Vec<SkinnedMeshRange>,
        vertex_buffer: VertexBuffer,
        index_buffer: IndexBuffer,
    ) -> Self {
        let aabb = AABB::from_vertex_iter(
            vertex_buffer
                .accessor::<Vector3<f32>>(ElementName::Position)
                .expect("vertex buffer must have position element")
                .iter(),
        );
        Self {
            bounding_sphere: aabb.bounding_sphere(),
            aabb,
            ranges,
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn aabb(&self) -> AABB<f32> {
        self.aabb
    }

    pub fn bounding_sphere(&self) -> Sphere {
        self.bounding_sphere
    }

    pub fn ranges(&self) -> &[SkinnedMeshRange] {
        &self.ranges
    }

    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &IndexBuffer {
        &self.index_buffer
    }
}

#[derive(TryFromPrimitive, IntoPrimitive, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(u32)]
enum SkinnedMeshVertexType {
    Basic,
    Color,
    Tangent,
}
