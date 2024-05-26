use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SkinnedMeshRange {
    material: String,
    start_vertex: i32,
    vertex_count: i32,
    start_index: i32,
    index_count: i32,
}

impl SkinnedMeshRange {
    pub fn new<S: Into<String>>(
        material: S,
        start_vertex: i32,
        vertex_count: i32,
        start_index: i32,
        index_count: i32,
    ) -> Self {
        Self {
            material: material.into(),
            start_vertex,
            vertex_count,
            start_index,
            index_count,
        }
    }

    pub fn from_reader<R: Read>(reader: &mut R) -> super::Result<Self> {
        use crate::core::mesh::r#static::ReaderExt;
        use byteorder::{LittleEndian, ReadBytesExt};
        Ok(Self {
            material: reader.read_padded_string::<LittleEndian, 64>()?,
            start_vertex: reader.read_i32::<LittleEndian>()?,
            vertex_count: reader.read_i32::<LittleEndian>()?,
            start_index: reader.read_i32::<LittleEndian>()?,
            index_count: reader.read_i32::<LittleEndian>()?,
        })
    }
}
