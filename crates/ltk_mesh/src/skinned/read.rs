use crate::{
    error::ParseError,
    mem::{IndexBuffer, VertexBuffer, VertexBufferDescription},
    skinned::{
        rebase_range_indices, vertex, SkinnedMeshFlags, SkinnedMeshVertexType, END_TAB_SIZE,
        MAX_VERTEX_COUNT, SKN_MAGIC,
    },
    SkinnedMesh, SkinnedMeshRange,
};
use byteorder::{ReadBytesExt, LE};
use ltk_io_ext::ReaderExt;
use ltk_primitives::{Sphere, AABB};
use num_enum::TryFromPrimitiveError;
use std::io::Read;

impl SkinnedMesh {
    /// Reads a `.skn` from a reader, applying the same checks and fixups the game does.
    ///
    /// Indices are normalized on the way in, so the mesh that comes back has one layout
    /// whatever the file used - see the [`SkinnedMesh`] docs.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidFileSignature`] if the magic is wrong,
    /// [`ParseError::InvalidFileVersion`] for a version outside `{0.1, 1.1, 2.1, 4.1}`,
    /// [`ParseError::InvalidField`] for a vertex type the format does not define, a
    /// `vertexSize` that disagrees with it, or a vertex count past [`MAX_VERTEX_COUNT`]
    /// without [`SkinnedMeshFlags::NORMALIZED_INDICES`], and [`ParseError::IOError`] on a
    /// short read.
    /// Every one of these is a file the game would refuse too.
    ///
    /// # Examples
    /// ```no_run
    /// use std::{fs::File, io::BufReader};
    /// use ltk_mesh::SkinnedMesh;
    ///
    /// let mesh = SkinnedMesh::from_reader(&mut BufReader::new(File::open("champion.skn")?))?;
    /// println!("{} submeshes", mesh.ranges().len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::Result<Self> {
        let magic = reader.read_u32::<LE>()?;
        if magic != SKN_MAGIC {
            return Err(ParseError::InvalidFileSignature);
        }

        // The game compares the whole version dword against {0x10000, 0x10001, 0x10002,
        // 0x10004}, so major 3 is rejected and minor must be exactly 1. Major 1 is shipped.
        let major = reader.read_u16::<LE>()?;
        let minor = reader.read_u16::<LE>()?;
        if !matches!(major, 0 | 1 | 2 | 4) || minor != 1 {
            return Err(ParseError::InvalidFileVersion(major, minor));
        }

        let index_count;
        let vertex_count;
        let ranges;
        let mut flags = SkinnedMeshFlags::empty();
        let mut vertex_declaration: VertexBufferDescription = vertex::BASIC.clone();
        let mut bounds: Option<(AABB, Sphere)> = None;

        if major == 0 {
            index_count = reader.read_u32::<LE>()?;
            vertex_count = reader.read_u32::<LE>()?;
            ranges = vec![SkinnedMeshRange::new(
                "",
                0,
                vertex_count as i32,
                0,
                index_count as i32,
            )];
        } else {
            let range_len = reader.read_u32::<LE>()?;
            ranges = (0..range_len)
                .map(|_| SkinnedMeshRange::from_reader(reader))
                .collect::<crate::Result<Vec<_>>>()?;

            if major == 4 {
                flags = SkinnedMeshFlags::from_bits_retain(reader.read_u32::<LE>()?);
                index_count = reader.read_u32::<LE>()?;
                vertex_count = reader.read_u32::<LE>()?;

                let vertex_size = reader.read_u32::<LE>()?;
                let vertex_type: SkinnedMeshVertexType = reader
                    .read_u32::<LE>()?
                    .try_into()
                    .map_err(|e: TryFromPrimitiveError<SkinnedMeshVertexType>| {
                        ParseError::InvalidField("vertex type", e.number.to_string())
                    })?;

                vertex_declaration = vertex_type.into();
                if vertex_size as usize != vertex_declaration.vertex_size() {
                    return Err(ParseError::InvalidField(
                        "vertex type/size",
                        format!("{vertex_type:?}: {vertex_size}"),
                    ));
                }

                bounds = Some((reader.read_aabb::<LE>()?, reader.read_sphere::<LE>()?));
            } else {
                index_count = reader.read_u32::<LE>()?;
                vertex_count = reader.read_u32::<LE>()?;
            }
        }

        // Absolute u16 indices cannot name a vertex past 65535, so the game rejects the file
        // unless it declares its indices already normalized.
        if vertex_count > MAX_VERTEX_COUNT && !flags.contains(SkinnedMeshFlags::NORMALIZED_INDICES)
        {
            return Err(ParseError::InvalidField(
                "vertex count",
                format!("{vertex_count} (max {MAX_VERTEX_COUNT} without NORMALIZED_INDICES)"),
            ));
        }

        let direct_blend_index_block = flags
            .contains(SkinnedMeshFlags::DIRECT_BLEND_INDICES)
            .then(|| -> std::io::Result<Vec<u8>> {
                let size = reader.read_u16::<LE>()? as usize;
                let mut block = vec![0; size];
                reader.read_exact(&mut block)?;
                Ok(block)
            })
            .transpose()?;

        // rebase indices by default
        let mut index_buffer = IndexBuffer::<u16>::read(reader, index_count as _)?;
        if !flags.contains(SkinnedMeshFlags::NORMALIZED_INDICES) {
            rebase_range_indices(&mut index_buffer, &ranges, u16::wrapping_sub);
        }

        let mut vertex_buffer = vec![0; vertex_declaration.vertex_size() * vertex_count as usize];
        reader.read_exact(&mut vertex_buffer)?;

        let mut end_tab = [0; END_TAB_SIZE];
        if major >= 2 {
            reader.read_exact(&mut end_tab)?;
        }

        let mut mesh = Self::new(
            ranges,
            VertexBuffer::new(vertex_declaration, vertex_buffer),
            index_buffer,
        );
        if let Some((aabb, bounding_sphere)) = bounds {
            mesh = mesh.with_bounds(aabb, bounding_sphere);
        }

        mesh.flags = flags;
        mesh.direct_blend_index_block = direct_blend_index_block;
        mesh.end_tab = end_tab;
        Ok(mesh)
    }
}
