use crate::{
    error::ParseError,
    skinned::{rebase_range_indices, SkinnedMeshFlags, MAX_VERTEX_COUNT, SKN_MAGIC},
    SkinnedMesh,
};
use byteorder::{WriteBytesExt, LE};
use ltk_io_ext::WriterExt;
use std::{borrow::Cow, io::Write};

impl SkinnedMesh {
    /// Writes the mesh as a v4.1 file.
    ///
    /// The stored indices are normalized; unless [`SkinnedMeshFlags::NORMALIZED_INDICES`] is
    /// set they are expanded back to absolute ones, which is the form every shipped file and
    /// every client uses.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] rather than emit a file the game would refuse:
    /// for a vertex buffer layout that matches no [`SkinnedMeshVertexType`], for more than
    /// [`MAX_VERTEX_COUNT`] vertices without [`SkinnedMeshFlags::NORMALIZED_INDICES`], or for
    /// a [`SkinnedMesh::direct_blend_index_block`] longer than [`u16::MAX`]. Returns
    /// [`ParseError::IOError`] if the writer fails.
    ///
    /// [`SkinnedMeshVertexType`]: crate::SkinnedMeshVertexType
    ///
    /// # Examples
    /// ```no_run
    /// # use ltk_mesh::SkinnedMesh;
    /// # fn demo(mesh: &SkinnedMesh) -> Result<(), Box<dyn std::error::Error>> {
    /// let mut bytes = Vec::new();
    /// mesh.to_writer(&mut bytes)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn to_writer<W: Write>(&self, w: &mut W) -> crate::Result<()> {
        let vertex_type = self.vertex_type().ok_or(ParseError::InvalidField(
            "vertex type",
            "vertex buffer layout does not match any skinned mesh vertex type".to_string(),
        ))?;

        let vertex_count = self.vertex_buffer.count();
        if vertex_count > MAX_VERTEX_COUNT as usize && !self.stores_normalized_indices() {
            return Err(ParseError::InvalidField(
                "vertex count",
                format!("{vertex_count} (max {MAX_VERTEX_COUNT} without NORMALIZED_INDICES)"),
            ));
        }

        // Guarded by the check above: expanding can only overflow a u16 index past
        // MAX_VERTEX_COUNT vertices, which is exactly the case that just errored out.
        let index_buffer = if self.stores_normalized_indices() {
            Cow::Borrowed(&self.index_buffer)
        } else {
            let mut absolute = self.index_buffer.clone();
            rebase_range_indices(&mut absolute, &self.ranges, u16::wrapping_add);
            Cow::Owned(absolute)
        };

        let block_size = self
            .direct_blend_index_block
            .as_ref()
            .map(|b| {
                u16::try_from(b.len()).map_err(|_| {
                    ParseError::InvalidField("direct blend index block", b.len().to_string())
                })
            })
            .transpose()?;

        w.write_u32::<LE>(SKN_MAGIC)?;

        w.write_u16::<LE>(4)?; // major
        w.write_u16::<LE>(1)?; // minor

        w.write_u32::<LE>(self.ranges.len() as u32)?;

        for range in &self.ranges {
            range.to_writer(w)?;
        }

        w.write_u32::<LE>(self.flags.bits())?;
        w.write_u32::<LE>(index_buffer.count() as u32)?;
        w.write_u32::<LE>(vertex_count as u32)?;
        w.write_u32::<LE>(self.vertex_buffer.stride() as u32)?;
        w.write_u32::<LE>(vertex_type.into())?;

        w.write_aabb::<LE>(&self.aabb)?;
        w.write_sphere::<LE>(&self.bounding_sphere)?;

        // Sits between the header and the index buffer, gated on DIRECT_BLEND_INDICES.
        if let (Some(size), Some(block)) = (block_size, &self.direct_blend_index_block) {
            debug_assert!(self.flags.contains(SkinnedMeshFlags::DIRECT_BLEND_INDICES));
            w.write_u16::<LE>(size)?;
            w.write_all(block)?;
        }

        w.write_all(index_buffer.as_bytes())?;
        w.write_all(self.vertex_buffer.as_bytes())?;

        w.write_all(&self.end_tab)?;
        Ok(())
    }
}
