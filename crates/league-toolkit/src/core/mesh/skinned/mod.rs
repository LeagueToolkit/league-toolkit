use std::io::Read;

use crate::core::{
    mem::{
        ElementName, IndexBuffer, IndexFormat, VertexBuffer, VertexBufferDescription, VertexElement,
    },
    primitives::{Sphere, AABB},
};

use byteorder::{LittleEndian, ReadBytesExt};
use num_enum::{FromPrimitive, TryFromPrimitive};

use super::{ParseError, Result};

mod range;
pub use range::*;

mod vertex;

const MAGIC: u32 = 0x00112233;

#[derive(Debug)]
pub struct SkinnedMesh {
    aabb: AABB<f32>,
    bounding_sphere: Sphere,
    ranges: Vec<SkinnedMeshRange>,
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,
    is_disposed: bool,
}

impl SkinnedMesh {
    pub fn new(
        ranges: Vec<SkinnedMeshRange>,
        vertex_buffer: VertexBuffer,
        index_buffer: IndexBuffer,
    ) -> Self {
        let aabb = AABB::from_vertex_iter(
            vertex_buffer
                .view(ElementName::Position)
                .expect("vertex buffer must have position element")
                .as_vec3()
                .iter(),
        );
        Self {
            bounding_sphere: aabb.bounding_sphere(),
            aabb,
            ranges,
            vertex_buffer,
            index_buffer,
            is_disposed: false,
        }
    }

    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        use super::ReaderExt as _;
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic != MAGIC {
            return Err(ParseError::InvalidFileSignature);
        }

        let major = reader.read_u16::<LittleEndian>()?;
        let minor = reader.read_u16::<LittleEndian>()?;
        if major != 0 && major != 2 && major != 4 && minor != 1 {
            return Err(ParseError::InvalidFileVersion);
        }

        let mut index_count = 0;
        let mut vertex_count = 0;
        let mut ranges;
        let mut vertex_declaration: VertexBufferDescription = vertex::BASIC.clone();
        let mut b_box = AABB::new();
        let mut b_sphere = Sphere::INFINITE;

        if major == 0 {
            index_count = reader.read_i32::<LittleEndian>()?;
            vertex_count = reader.read_i32::<LittleEndian>()?;
            ranges = vec![SkinnedMeshRange::new("Base", 0, 0, 0, 0)] // TODO (alan): impl SkinnedMeshRange
        } else {
            let range_len = reader.read_u32::<LittleEndian>()? as usize;
            ranges = Vec::with_capacity(range_len);
            for _ in 0..range_len {
                ranges.push(SkinnedMeshRange::from_reader(reader)?) // TODO (alan): impl SkinnedMeshRange
            }

            if major == 4 {
                let _flags = reader.read_u32::<LittleEndian>()?;
            }

            index_count = reader.read_i32::<LittleEndian>()?;
            vertex_count = reader.read_i32::<LittleEndian>()?;

            if major == 4 {
                let vertex_size = reader.read_u32::<LittleEndian>()?;
                let vertex_type: SkinnedMeshVertexType = reader
                    .read_u32::<LittleEndian>()?
                    .try_into()
                    .expect("invalid vertex type"); // TODO (alan): handle TryFromPrimitive error?
                vertex_declaration = match (vertex_size, vertex_type) {
                    (52, SkinnedMeshVertexType::Basic) => vertex::BASIC.clone(),
                    (56, SkinnedMeshVertexType::Color) => vertex::COLOR.clone(),
                    (72, SkinnedMeshVertexType::Tangent) => vertex::TANGENT.clone(),
                    _ => {
                        return Err(ParseError::InvalidFileSignature); // TODO (alan): real error here
                    }
                };

                b_box = reader.read_bbox_f32::<LittleEndian>()?;
                b_sphere = reader.read_sphere_f32::<LittleEndian>()?;
            }
        }

        let index_buffer = IndexBuffer::new(
            crate::core::mem::IndexFormat::U16,
            vec![0; (index_count as usize) * IndexFormat::U16.size()],
        );
        let vertex_buffer = vertex_declaration.into_vertex_buffer(vertex_count as usize);

        Ok(Self::new(ranges, vertex_buffer, index_buffer))
    }
}

#[derive(TryFromPrimitive)]
#[repr(u32)]
enum SkinnedMeshVertexType {
    Basic,
    Color,
    Tangent,
}
