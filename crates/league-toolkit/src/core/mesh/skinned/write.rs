use core::borrow;
use std::cmp::PartialEq;
use std::io::{Write};
use byteorder::{LittleEndian, WriteBytesExt};
use num_enum::TryFromPrimitiveError;
use crate::core::mem::{IndexBuffer, IndexFormat, VertexBufferDescription};
use crate::core::mesh::error::ParseError;
use crate::core::mesh::skinned::{MAGIC, SkinnedMeshVertexType, vertex};
use crate::core::mesh::{SkinnedMesh, SkinnedMeshRange};
use crate::core::primitives::{AABB, Sphere};
use crate::util::WriterExt;

impl SkinnedMesh {
    pub fn to_writer<W: Write>(&self, w: &mut W) -> crate::core::mesh::Result<()> {
        use crate::util::WriterExt as _;
        w.write_u32::<LittleEndian>(MAGIC)?;

        w.write_u16::<LittleEndian>(4)?; // major
        w.write_u16::<LittleEndian>(1)?; // minor

        w.write_i32::<LittleEndian>(self.ranges.len() as i32)?;

        for range in &self.ranges {
            range.to_writer(w)?;
        }

        w.write_u32::<LittleEndian>(0)?; // flags
        w.write_i32::<LittleEndian>(self.index_buffer.count() as i32)?;
        w.write_i32::<LittleEndian>(self.vertex_buffer.count() as i32)?;
        w.write_u32::<LittleEndian>(self.vertex_buffer.stride() as u32)?;
        w.write_u32::<LittleEndian>(match self.vertex_buffer.description() {
            d if d == &*vertex::BASIC => SkinnedMeshVertexType::Basic.into(),
            d if d == &*vertex::COLOR => SkinnedMeshVertexType::Color.into(),
            d if d == &*vertex::TANGENT => SkinnedMeshVertexType::Tangent.into(),
            _ => panic!("FIXME: unhandled mesh vertex type"),
        })?;

        w.write_aabb::<LittleEndian>(&self.aabb)?;
        w.write_sphere::<LittleEndian>(&self.bounding_sphere)?;

        w.write_all(self.index_buffer.buffer())?;
        w.write_all(self.vertex_buffer.buffer())?;

        w.write_all(&[0_u8; 12])?; // end tab
        Ok(())
    }
}
