use std::io::Read;
use byteorder::{LittleEndian, ReadBytesExt};
use num_enum::TryFromPrimitiveError;
use crate::core::mem::{IndexBuffer, IndexFormat, VertexBufferDescription};
use crate::core::mesh::error::ParseError;
use crate::core::mesh::skinned::{MAGIC, SkinnedMeshVertexType, vertex};
use crate::core::mesh::{SkinnedMesh, SkinnedMeshRange};
use crate::core::primitives::{AABB, Sphere};

impl SkinnedMesh {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::core::mesh::Result<Self> {
        use crate::util::ReaderExt as _;
        let magic = reader.read_u32::<LittleEndian>()?;
        if magic != MAGIC {
            return Err(ParseError::InvalidFileSignature);
        }

        let major = reader.read_u16::<LittleEndian>()?;
        let minor = reader.read_u16::<LittleEndian>()?;
        if major != 0 && major != 2 && major != 4 && minor != 1 {
            return Err(ParseError::InvalidFileVersion(major, minor));
        }

        let index_count;
        let vertex_count;
        let mut ranges;
        let mut vertex_declaration: VertexBufferDescription = vertex::BASIC.clone();
        let mut _b_box = AABB::default();
        let mut _b_sphere = Sphere::INFINITE;

        if major == 0 {
            index_count = reader.read_i32::<LittleEndian>()?;
            vertex_count = reader.read_i32::<LittleEndian>()?;
            ranges = vec![SkinnedMeshRange::new("Base", 0, 0, 0, 0)]
        } else {
            let range_len = reader.read_u32::<LittleEndian>()? as usize;
            ranges = Vec::with_capacity(range_len);
            for _ in 0..range_len {
                ranges.push(SkinnedMeshRange::from_reader(reader)?)
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
                    .map_err(|e: TryFromPrimitiveError<SkinnedMeshVertexType>| ParseError::InvalidField("vertex type", e.number.to_string()))?;
                vertex_declaration = match (vertex_size, vertex_type) {
                    (52, SkinnedMeshVertexType::Basic) => vertex::BASIC.clone(),
                    (56, SkinnedMeshVertexType::Color) => vertex::COLOR.clone(),
                    (72, SkinnedMeshVertexType::Tangent) => vertex::TANGENT.clone(),
                    _ => {
                        return Err(ParseError::InvalidField("vertex type/size", format!("{vertex_type:?}: {vertex_size}")));
                    }
                };

                _b_box = reader.read_aabb::<LittleEndian>()?;
                _b_sphere = reader.read_sphere::<LittleEndian>()?;
            }
        }

        let mut index_buffer = vec![0; (index_count as usize) * IndexFormat::U16.size()];
        reader.read_exact(&mut index_buffer)?;
        let index_buffer = IndexBuffer::new(crate::core::mem::IndexFormat::U16, index_buffer);

        let mut vertex_buffer = vec![0; vertex_declaration.vertex_size() * vertex_count as usize];
        reader.read_exact(&mut vertex_buffer)?;
        let vertex_buffer = vertex_declaration.into_vertex_buffer(vertex_buffer);

        Ok(Self::new(ranges, vertex_buffer, index_buffer))
    }
}
