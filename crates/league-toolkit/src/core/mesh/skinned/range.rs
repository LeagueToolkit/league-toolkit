use std::io;
use std::io::{Read, Write};
use byteorder::{LittleEndian, WriteBytesExt};
use crate::util::WriterExt;

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
        use crate::util::ReaderExt as _;
        use byteorder::{LittleEndian, ReadBytesExt};
        Ok(Self {
            material: reader.read_padded_string::<LittleEndian, 64>()?,
            start_vertex: reader.read_i32::<LittleEndian>()?,
            vertex_count: reader.read_i32::<LittleEndian>()?,
            start_index: reader.read_i32::<LittleEndian>()?,
            index_count: reader.read_i32::<LittleEndian>()?,
        })
    }

    pub fn to_writer<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        use crate::util::WriterExt;
        writer.write_padded_string::<64>(&self.material)?;
        writer.write_i32::<LittleEndian>(self.start_vertex)?;
        writer.write_i32::<LittleEndian>(self.vertex_count)?;
        writer.write_i32::<LittleEndian>(self.start_index)?;
        writer.write_i32::<LittleEndian>(self.index_count)?;
        Ok(())
    }
}
