use std::io::Write;

use byteorder::{WriteBytesExt, LE};
use ltk_io_ext::WriterExt;

use crate::{
    error::ParseError,
    render::{RenderMesh, RenderMeshSubmesh, VERSION},
};

impl RenderMesh {
    /// Writes the mesh as a version 1 file.
    ///
    /// The writer writes [`RenderMesh::magic`] and the stored bounding box without change.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] for a vertex count, index count, stream count,
    /// buffer size, submesh count or material name length past [`u32::MAX`], and for a vertex
    /// layout of more than
    /// [`VertexBufferDescription::SERIALIZED_ELEMENT_SLOTS`](crate::mem::VertexBufferDescription::SERIALIZED_ELEMENT_SLOTS)
    /// elements. Returns [`ParseError::IOError`] if the writer fails.
    ///
    /// # Examples
    /// ```no_run
    /// # use ltk_mesh::RenderMesh;
    /// # fn demo(mesh: &RenderMesh) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut bytes = Vec::new();
    /// mesh.to_writer(&mut bytes)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> crate::Result<()> {
        writer.write_all(&self.magic)?;
        writer.write_u32::<LE>(VERSION)?;
        write_count(writer, self.vertex_count, "vertex count")?;
        write_count(writer, self.index_buffer.count(), "index count")?;
        writer.write_aabb::<LE>(&self.bounding_box)?;

        write_count(writer, self.vertex_buffers.len(), "vertex stream count")?;
        for buffer in &self.vertex_buffers {
            buffer.description().to_writer(writer)?;
        }
        for buffer in &self.vertex_buffers {
            write_count(writer, buffer.as_bytes().len(), "vertex buffer size")?;
            writer.write_all(buffer.as_bytes())?;
        }

        writer.write_all(self.index_buffer.as_bytes())?;

        write_count(writer, self.submeshes.len(), "submesh count")?;
        for submesh in &self.submeshes {
            submesh.to_writer(writer)?;
        }
        Ok(())
    }
}

impl RenderMeshSubmesh {
    fn to_writer<W: Write>(&self, writer: &mut W) -> crate::Result<()> {
        write_count(writer, self.material.len(), "material name length")?;
        writer.write_all(self.material.as_bytes())?;
        writer.write_u32::<LE>(self.start_index)?;
        writer.write_u32::<LE>(self.index_count)?;
        writer.write_u32::<LE>(self.min_vertex)?;
        writer.write_u32::<LE>(self.max_vertex)?;
        Ok(())
    }
}

fn write_count<W: Write>(writer: &mut W, count: usize, what: &'static str) -> crate::Result<()> {
    let count =
        u32::try_from(count).map_err(|_| ParseError::InvalidField(what, count.to_string()))?;
    writer.write_u32::<LE>(count)?;
    Ok(())
}
