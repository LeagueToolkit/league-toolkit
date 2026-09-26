use std::io::Read;

use byteorder::{ReadBytesExt, LE};
use ltk_io_ext::{
    untrusted::{read_bytes, UntrustedCapacity},
    ReaderExt,
};

use crate::{
    error::ParseError,
    mem::{IndexBuffer, VertexBuffer, VertexBufferDescription},
    render::{check_streams, RenderMesh, RenderMeshSubmesh, VERSION},
};

impl RenderMesh {
    /// Reads a `.gmesh` or `.tmesh` from a reader.
    ///
    /// The reader accepts any magic. The game does not check the magic.
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] for a version other than 1, for a vertex buffer
    /// description [`VertexBufferDescription::from_reader`] rejects, for a vertex buffer whose
    /// byte size is not its stride times the vertex count, and for any mesh
    /// [`RenderMesh::new`] rejects. Returns [`ParseError::Utf8Error`] for a material name that
    /// is not UTF-8, and [`ParseError::IOError`] on a short read.
    ///
    /// # Examples
    /// ```no_run
    /// use std::{fs::File, io::BufReader};
    /// use ltk_mesh::RenderMesh;
    ///
    /// let mesh = RenderMesh::from_reader(&mut BufReader::new(File::open("accent.gmesh")?))?;
    /// println!("{} vertices", mesh.vertex_count());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::Result<Self> {
        let mut magic = [0; 4];
        reader.read_exact(&mut magic)?;

        let version = reader.read_u32::<LE>()?;
        if version != VERSION {
            return Err(ParseError::InvalidField("version", version.to_string()));
        }

        let vertex_count = reader.read_u32::<LE>()? as usize;
        let index_count = reader.read_u32::<LE>()? as usize;
        let bounding_box = reader.read_aabb::<LE>()?;

        let stream_count = reader.read_u32::<LE>()? as usize;
        let mut descriptions = Vec::with_untrusted_capacity(stream_count);
        for _ in 0..stream_count {
            descriptions.push(VertexBufferDescription::from_reader(reader)?);
        }

        let mut vertex_buffers = Vec::with_capacity(descriptions.len());
        for (stream, description) in descriptions.into_iter().enumerate() {
            let byte_size = reader.read_u32::<LE>()? as usize;
            let expected = description.vertex_size().checked_mul(vertex_count);
            if expected != Some(byte_size) {
                return Err(ParseError::InvalidField(
                    "vertex buffer size",
                    format!(
                        "stream {stream} is {byte_size} bytes, {vertex_count} vertices of {} bytes",
                        description.vertex_size()
                    ),
                ));
            }
            vertex_buffers.push(VertexBuffer::new(
                description,
                read_bytes(reader, byte_size)?,
            ));
        }
        check_streams(&vertex_buffers)?;

        let index_bytes = index_count
            .checked_mul(2)
            .ok_or_else(|| ParseError::InvalidField("index count", index_count.to_string()))?;
        let index_buffer = IndexBuffer::<u16>::new(read_bytes(reader, index_bytes)?);

        let submesh_count = reader.read_u32::<LE>()? as usize;
        let mut submeshes = Vec::with_untrusted_capacity(submesh_count);
        for _ in 0..submesh_count {
            submeshes.push(RenderMeshSubmesh::from_reader(reader)?);
        }

        Ok(Self {
            magic,
            bounding_box,
            vertex_count,
            vertex_buffers,
            index_buffer,
            submeshes,
        })
    }
}

impl RenderMeshSubmesh {
    fn from_reader<R: Read>(reader: &mut R) -> crate::Result<Self> {
        let length = reader.read_u32::<LE>()? as usize;
        let material = String::from_utf8(read_bytes(reader, length)?)
            .map_err(|e| ParseError::Utf8Error(e.utf8_error()))?;
        Ok(Self {
            material,
            start_index: reader.read_u32::<LE>()?,
            index_count: reader.read_u32::<LE>()?,
            min_vertex: reader.read_u32::<LE>()?,
            max_vertex: reader.read_u32::<LE>()?,
        })
    }
}
