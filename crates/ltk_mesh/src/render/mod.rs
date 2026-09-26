//! Render meshes - the `.gmesh` and `.tmesh` formats.
//!
//! Private: every item here is re-exported at the crate root, which is its only public path.
//! User facing documentation lives on [`RenderMesh`] itself.
use ltk_primitives::AABB;

use crate::{
    error::ParseError,
    mem::vertex::ElementName,
    mem::{
        vertex::{get_element_flags, Format, VertexBufferElementFlags},
        IndexBuffer, VertexBuffer, VertexBufferAccessor,
    },
};

mod read;
mod write;

/// The first four bytes of every shipped `.gmesh` file.
///
/// The game does not compare the magic against a constant. The magic of a `.tmesh` file is
/// unknown.
pub const GMESH_MAGIC: [u8; 4] = *b"GMSH";

/// The only file version the game accepts.
const VERSION: u32 = 1;

/// A GPU-ready mesh, as stored in a `.gmesh` or `.tmesh` file.
///
/// The file is a serialized `Riot::Renderer::Mesh`: one or more vertex streams, a `u16` index
/// buffer, a bounding box, and a table of [`RenderMeshSubmesh`]es. The game parses `.gmesh`
/// and `.tmesh` with the same code. A shipped `.gmesh` has the vertex layout of map
/// geometry: positions alone in a `f32` stream, and the normal, UVs, tangent and lightmap UV
/// interleaved as halves in a second stream.
///
/// Every vertex stream holds [`RenderMesh::vertex_count`] vertices. An element name appears in
/// at most one stream. [`RenderMesh::accessor`] finds an element in any stream.
///
/// Read one with [`RenderMesh::from_reader`] and write one with [`RenderMesh::to_writer`].
/// A shipped file round-trips byte for byte.
///
/// # File layout
///
/// All values are little endian.
///
/// ```text
/// [4]      magic               "GMSH" in every shipped .gmesh, not checked by the game
/// u32      version             must be 1
/// u32      vertex count        shared by every stream
/// u32      index count
/// f32 x 6  bounding box        min xyz, then max xyz
/// u32      stream count
/// stream count x 128 bytes     vertex buffer descriptions
/// stream count x { u32 byte size, byte size bytes }
/// u16 x index count            indices
/// u32      submesh count
/// submesh count x { u32 length, length bytes of material name,
///                   u32 start index, u32 index count, u32 min vertex, u32 max vertex }
/// ```
///
/// A description is the 128-byte layout
/// [`VertexBufferDescription::from_reader`](crate::mem::VertexBufferDescription::from_reader)
/// reads.
///
/// # Examples
/// ```no_run
/// use std::{fs::File, io::BufReader};
///
/// use glam::Vec3;
/// use ltk_mesh::{mem::vertex::ElementName, RenderMesh};
///
/// let mesh = RenderMesh::from_reader(&mut BufReader::new(File::open("accent.gmesh")?))?;
/// let positions = mesh
///     .accessor::<Vec3>(ElementName::Position)
///     .expect("every shipped .gmesh carries positions");
///
/// for submesh in mesh.submeshes() {
///     let first = mesh.index_buffer().get(submesh.start_index as usize);
///     println!("{}: starts at {}", submesh.material, positions.get(first as usize));
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RenderMesh {
    magic: [u8; 4],
    bounding_box: AABB,
    vertex_count: usize,
    vertex_buffers: Vec<VertexBuffer>,
    index_buffer: IndexBuffer<u16>,
    submeshes: Vec<RenderMeshSubmesh>,
}

/// A submesh: a range of the index buffer and its material.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RenderMeshSubmesh {
    /// Name of the material of this submesh.
    pub material: String,
    /// First index of this submesh in the index buffer.
    pub start_index: u32,
    /// Number of indices in this submesh.
    pub index_count: u32,
    /// Lowest vertex index in this submesh, inclusive.
    pub min_vertex: u32,
    /// Highest vertex index in this submesh, inclusive.
    pub max_vertex: u32,
}

impl RenderMesh {
    /// Creates a mesh from its vertex streams, indices and submeshes.
    ///
    /// The magic is [`GMESH_MAGIC`]. Replace it with [`RenderMesh::with_magic`].
    ///
    /// # Errors
    /// Returns [`ParseError::InvalidField`] if `vertex_buffers` is empty, if two streams hold
    /// a different number of vertices, or if two streams hold the same element name.
    pub fn new(
        bounding_box: AABB,
        vertex_buffers: Vec<VertexBuffer>,
        index_buffer: IndexBuffer<u16>,
        submeshes: Vec<RenderMeshSubmesh>,
    ) -> crate::Result<Self> {
        let vertex_count = check_streams(&vertex_buffers)?;
        Ok(Self {
            magic: GMESH_MAGIC,
            bounding_box,
            vertex_count,
            vertex_buffers,
            index_buffer,
            submeshes,
        })
    }

    /// Replaces the four magic bytes that the writer writes.
    #[must_use]
    pub fn with_magic(mut self, magic: [u8; 4]) -> Self {
        self.magic = magic;
        self
    }

    /// The four magic bytes at the start of the file.
    #[must_use]
    pub fn magic(&self) -> [u8; 4] {
        self.magic
    }

    /// The bounding box the file stores.
    ///
    /// The reader does not recompute it from the positions.
    #[must_use]
    pub fn bounding_box(&self) -> AABB {
        self.bounding_box
    }

    /// The number of vertices in every stream.
    #[must_use]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// The vertex streams, in file order.
    #[must_use]
    pub fn vertex_buffers(&self) -> &[VertexBuffer] {
        &self.vertex_buffers
    }

    /// The index buffer of all submeshes.
    #[must_use]
    pub fn index_buffer(&self) -> &IndexBuffer<u16> {
        &self.index_buffer
    }

    /// The submeshes, in file order.
    #[must_use]
    pub fn submeshes(&self) -> &[RenderMeshSubmesh] {
        &self.submeshes
    }

    /// An accessor for one vertex element, in the stream that has the element.
    ///
    /// Returns [`None`] if no stream has the element, or if `T` does not decode its format.
    /// [`VertexBuffer::accessor`] lists the formats each `T` decodes.
    #[must_use]
    pub fn accessor<T: Format>(
        &self,
        element_name: ElementName,
    ) -> Option<VertexBufferAccessor<'_, T>> {
        self.vertex_buffers
            .iter()
            .find(|buffer| buffer.elements().contains_key(&element_name))?
            .accessor(element_name)
    }
}

/// Returns the vertex count every stream shares.
///
/// Errors on no streams, on streams of different lengths, and on an element name that is in two
/// streams. The game rejects a mesh with a repeated element name.
fn check_streams(vertex_buffers: &[VertexBuffer]) -> crate::Result<usize> {
    let first = vertex_buffers
        .first()
        .ok_or_else(|| ParseError::InvalidField("vertex stream count", "0".to_string()))?;

    let mut names = VertexBufferElementFlags::empty();
    for (stream, buffer) in vertex_buffers.iter().enumerate() {
        if buffer.count() != first.count() {
            return Err(ParseError::InvalidField(
                "vertex stream length",
                format!(
                    "stream {stream} holds {} vertices, stream 0 holds {}",
                    buffer.count(),
                    first.count()
                ),
            ));
        }

        let flags = get_element_flags(buffer.description().elements().iter().map(|e| e.name));
        if names.intersects(flags) {
            return Err(ParseError::InvalidField(
                "vertex element name",
                format!("{:?} appears in two streams", names & flags),
            ));
        }
        names |= flags;
    }
    Ok(first.count())
}
