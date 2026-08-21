//! Skinned meshes - the `.skn` (Simple Skin) format.
//!
//! Private: every item here is re-exported at the crate root, which is its only public path.
//! User facing documentation lives on [`SkinnedMesh`] itself.
use glam::Vec3;
use num_enum::{IntoPrimitive, TryFromPrimitive};

#[doc(inline)]
pub use range::SkinnedMeshRange;

use crate::mem::{
    index::IndexBuffer,
    vertex::{ElementName, VertexBuffer, VertexBufferDescription},
};
use ltk_primitives::{Sphere, AABB};

use super::Result;

mod range;
mod read;
mod vertex;
mod write;

/// Magic bytes of every `.skn` file.
pub const SKN_MAGIC: u32 = 0x0011_2233;

/// Max number of vertices for the whole mesh.
/// Enabling [`SkinnedMeshFlags::NORMALIZED_INDICES`] lifts the limit and makes the mesh use normalized indices per range.
pub const MAX_VERTEX_COUNT: u32 = 0x10000;

/// The 12 byte tail carried by every `major >= 2` file. Read last, after the vertex buffer.
pub const END_TAB_SIZE: usize = 12;

bitflags::bitflags! {
    /// The `flags` word of the v4 `.skn` header.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SkinnedMeshFlags: u32 {
        /// Blend indices are used as-is, skipping the influence table remap normally applied when binding to a rig.
        /// Draws the mesh as a single skinning partition.
        ///
        /// A file with this bit also carries an extra `u16` length prefixed block between the
        /// header and the index buffer, see [`SkinnedMesh::direct_blend_index_block`].
        const DIRECT_BLEND_INDICES = 1;

        /// The index buffer is stored normalized, with every index relative to the `start_vertex` of the range that owns it.
        ///
        /// This also lifts [`MAX_VERTEX_COUNT`], since a normalized `u16` index is resolved as
        /// `start_vertex + index` in 32 bits. Every range must still be 65536 vertices or
        /// fewer.
        ///
        /// A [`SkinnedMesh`] is always normalized in memory, so this bit only decides which
        /// form [`SkinnedMesh::to_writer`] emits - see
        /// [`SkinnedMesh::stores_normalized_indices`].
        const NORMALIZED_INDICES = 2;
    }
}

impl Default for SkinnedMeshFlags {
    fn default() -> Self {
        Self::empty()
    }
}

/// A skinned mesh, as stored in a `.skn` (Simple Skin) file.
///
/// One vertex buffer and one index buffer are shared by every [`SkinnedMeshRange`]
/// (submesh); a range owns the index slice `start_index .. start_index + index_count` and
/// the vertex slice `start_vertex .. start_vertex + vertex_count`. Each vertex additionally
/// carries the four blend indices and four blend weights that bind it to a rig.
///
/// The index buffer is always **normalized**: every index is relative to the `start_vertex`
/// of the range that owns it, whichever way the file on disk stored it. This is the same
/// preprocessing the game does on load, and it means a consumer never has to branch on
/// [`SkinnedMeshFlags::NORMALIZED_INDICES`].
///
/// # Versions
///
/// [`SkinnedMesh::from_reader`] accepts `0.1`, `1.1`, `2.1` and `4.1`, matching the set the
/// game itself takes. Only `4.1` carries [`SkinnedMeshFlags`], an explicit vertex layout and
/// stored bounds; older versions have their bounds computed from the vertex buffer, and `0.1`
/// gets a single unnamed range spanning the whole mesh. [`SkinnedMesh::to_writer`] always
/// emits `4.1`.
///
/// # Reading the geometry
///
/// [`SkinnedMesh::range_indices`] yields a range's indices as absolute positions in the
/// shared vertex buffer, which is what you want on the CPU. Resolve each accessor **once**
/// and reuse it across the whole walk - it is a map lookup, and doing it per vertex is the
/// easiest way to make this slow.
///
/// ```
/// use glam::Vec3;
/// use ltk_mesh::{mem::vertex::ElementName, SkinnedMesh};
/// # use ltk_mesh::{mem::{IndexBuffer, VertexBuffer, VertexBufferDescription}, SkinnedMeshRange, SkinnedMeshVertexType};
/// # let mesh = SkinnedMesh::new(
/// #     vec![SkinnedMeshRange::new("body", 0, 4, 0, 6)],
/// #     VertexBuffer::new(VertexBufferDescription::from(SkinnedMeshVertexType::Basic), vec![0; 52 * 4]),
/// #     IndexBuffer::<u16>::new([0_u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect()),
/// # );
/// let positions = mesh
///     .vertex_buffer()
///     .accessor::<Vec3>(ElementName::Position)
///     .expect("every .skn carries positions");
///
/// for range in mesh.ranges() {
///     let mut indices = mesh.range_indices(range);
///     // Indices are a triangle list, so take them three at a time.
///     while let (Some(a), Some(b), Some(c)) = (indices.next(), indices.next(), indices.next()) {
///         let _triangle = [
///             positions.get(a as usize),
///             positions.get(b as usize),
///             positions.get(c as usize),
///         ];
///     }
/// }
/// ```
///
/// # Uploading to the GPU
///
/// Normalized indices are already in the form a graphics API wants, so nothing has to be
/// touched per index: upload both buffers as is and issue one indexed draw per range,
/// passing `start_vertex` as the base vertex. This is the cheapest way to consume a mesh and
/// the reason the in-memory buffer is normalized.
///
/// ```
/// # use ltk_mesh::SkinnedMesh;
/// # fn upload(_: &[u8]) {}
/// # fn draw_indexed(_index_count: i32, _first_index: i32, _base_vertex: i32) {}
/// # use ltk_mesh::{mem::{IndexBuffer, VertexBuffer, VertexBufferDescription}, SkinnedMeshRange, SkinnedMeshVertexType};
/// # let mesh = SkinnedMesh::new(
/// #     vec![SkinnedMeshRange::new("body", 0, 4, 0, 6)],
/// #     VertexBuffer::new(VertexBufferDescription::from(SkinnedMeshVertexType::Basic), vec![0; 52 * 4]),
/// #     IndexBuffer::<u16>::new([0_u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect()),
/// # );
/// // Both are contiguous and need no conversion - u16 indices, interleaved vertices.
/// upload(mesh.vertex_buffer().as_bytes());
/// upload(mesh.index_buffer().as_bytes());
///
/// for range in mesh.ranges() {
///     draw_indexed(range.index_count, range.start_index, range.start_vertex);
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct SkinnedMesh {
    flags: SkinnedMeshFlags,
    aabb: AABB,
    bounding_sphere: Sphere,
    ranges: Vec<SkinnedMeshRange>,
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer<u16>,
    direct_blend_index_block: Option<Vec<u8>>,
    end_tab: [u8; END_TAB_SIZE],
}

impl SkinnedMesh {
    /// Creates a mesh with no flags set, computing the bounds from the vertex buffer.
    ///
    /// `index_buffer` must already be normalized - see the type level docs. Use
    /// [`SkinnedMesh::from_absolute_indices`] if the indices point into the shared vertex
    /// buffer instead.
    ///
    /// Use [`SkinnedMesh::with_bounds`] to carry a file's own bounds instead of the computed
    /// ones.
    ///
    /// # Panics
    /// Panics if `vertex_buffer` has no [`ElementName::Position`] element, which no skinned
    /// mesh layout can omit.
    #[must_use]
    pub fn new(
        ranges: Vec<SkinnedMeshRange>,
        vertex_buffer: VertexBuffer,
        index_buffer: IndexBuffer<u16>,
    ) -> Self {
        let aabb = AABB::of_points(
            vertex_buffer
                .accessor::<Vec3>(ElementName::Position)
                .expect("vertex buffer must have position element")
                .iter(),
        );
        Self {
            flags: SkinnedMeshFlags::empty(),
            bounding_sphere: aabb.bounding_sphere(),
            aabb,
            ranges,
            vertex_buffer,
            index_buffer,
            direct_blend_index_block: None,
            end_tab: [0; END_TAB_SIZE],
        }
    }

    /// Creates a mesh from absolute indices, normalizing them against their ranges.
    ///
    /// Use this when the indices point straight into the shared vertex buffer, as they do in
    /// most interchange formats; [`SkinnedMesh::new`] takes already normalized ones.
    ///
    /// # Panics
    /// Panics if `vertex_buffer` has no [`ElementName::Position`] element.
    #[must_use]
    pub fn from_absolute_indices(
        ranges: Vec<SkinnedMeshRange>,
        vertex_buffer: VertexBuffer,
        mut index_buffer: IndexBuffer<u16>,
    ) -> Self {
        rebase_range_indices(&mut index_buffer, &ranges, u16::wrapping_sub);
        Self::new(ranges, vertex_buffer, index_buffer)
    }

    /// Overrides the computed bounds, e.g. to keep the ones stored in a v4 file.
    #[must_use]
    pub fn with_bounds(mut self, aabb: AABB, bounding_sphere: Sphere) -> Self {
        self.aabb = aabb;
        self.bounding_sphere = bounding_sphere;
        self
    }

    /// Bounding box of this mesh
    pub fn aabb(&self) -> AABB {
        self.aabb
    }

    /// Bounding sphere of this mesh
    pub fn bounding_sphere(&self) -> Sphere {
        self.bounding_sphere
    }

    /// The header flags. Only written for v4 files, which is what [`SkinnedMesh::to_writer`]
    /// always emits.
    pub fn flags(&self) -> SkinnedMeshFlags {
        self.flags
    }

    /// Sets the header flags.
    ///
    /// [`SkinnedMeshFlags::DIRECT_BLEND_INDICES`] cannot be set this way - it always mirrors
    /// whether a block is present, see [`SkinnedMesh::set_direct_blend_index_block`].
    pub fn set_flags(&mut self, flags: SkinnedMeshFlags) {
        self.flags = flags;
        self.sync_direct_blend_indices_flag();
    }

    /// Whether [`SkinnedMesh::to_writer`] keeps the normalized indices as they are, rather
    /// than expanding them back to absolute ones.
    ///
    /// Writing normalized is only necessary past [`MAX_VERTEX_COUNT`] vertices, where an absolute `u16` index cannot reach.
    pub fn stores_normalized_indices(&self) -> bool {
        self.flags.contains(SkinnedMeshFlags::NORMALIZED_INDICES)
    }

    /// The block carried by [`SkinnedMeshFlags::DIRECT_BLEND_INDICES`] files, framed on disk
    /// as `u16 size` followed by `size` bytes, positioned between the header and the index
    /// buffer.
    ///
    /// Its payload is opaque. Presumably used as data for a shader.
    pub fn direct_blend_index_block(&self) -> Option<&[u8]> {
        self.direct_blend_index_block.as_deref()
    }

    /// Sets the block described by [`SkinnedMesh::direct_blend_index_block`], keeping
    /// [`SkinnedMeshFlags::DIRECT_BLEND_INDICES`] in sync.
    ///
    /// The block may not exceed [`u16::MAX`] bytes; [`SkinnedMesh::to_writer`] fails if it
    /// does.
    pub fn set_direct_blend_index_block(&mut self, block: Option<Vec<u8>>) {
        self.direct_blend_index_block = block;
        self.sync_direct_blend_indices_flag();
    }

    /// The 12 byte tail of a `major >= 2` file.
    ///
    /// The game reads it - a short read makes the whole load fail - and then discards it. It
    /// is zero in every shipped file, and is kept here only so a round trip is byte exact.
    pub fn end_tab(&self) -> &[u8; END_TAB_SIZE] {
        &self.end_tab
    }

    /// The submeshes, in the order the file lists them.
    #[must_use]
    pub fn ranges(&self) -> &[SkinnedMeshRange] {
        &self.ranges
    }

    /// The shared vertex buffer, interleaved in the layout named by
    /// [`SkinnedMesh::vertex_type`].
    ///
    /// Position, blend indices, blend weights, normal and `Texcoord0` are present in every
    /// layout; colour, tangent and the extra UV channels only in some, so
    /// [`VertexBuffer::accessor`] returns [`None`] rather than making you check the type
    /// first.
    ///
    /// ```
    /// use glam::{Vec2, Vec3, Vec4};
    /// use ltk_mesh::{mem::vertex::ElementName, SkinnedMesh};
    /// # use ltk_mesh::{mem::{IndexBuffer, VertexBuffer, VertexBufferDescription}, SkinnedMeshRange, SkinnedMeshVertexType};
    /// # let mesh = SkinnedMesh::new(
    /// #     vec![SkinnedMeshRange::new("body", 0, 4, 0, 6)],
    /// #     VertexBuffer::new(VertexBufferDescription::from(SkinnedMeshVertexType::Basic), vec![0; 52 * 4]),
    /// #     IndexBuffer::<u16>::new([0_u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect()),
    /// # );
    /// let vertices = mesh.vertex_buffer();
    ///
    /// // Always there. Read as the type matching the element's format:
    /// // Vec3 position/normal, Vec2 uv, Vec4 blend weights, [u8; 4] blend indices.
    /// let normals = vertices.accessor::<Vec3>(ElementName::Normal).unwrap();
    /// let uvs = vertices.accessor::<Vec2>(ElementName::Texcoord0).unwrap();
    /// let blend_indices = vertices.accessor::<[u8; 4]>(ElementName::BlendIndex).unwrap();
    /// let blend_weights = vertices.accessor::<Vec4>(ElementName::BlendWeight).unwrap();
    ///
    /// // Layout dependent - absent on the 52 byte type.
    /// let tangents = vertices.accessor::<Vec4>(ElementName::Texcoord6);
    ///
    /// // iter() walks the whole buffer, which is what you want for a per-vertex pass.
    /// for (indices, weights) in blend_indices.iter().zip(blend_weights.iter()) {
    ///     let _influences = indices.iter().zip(weights.to_array()).filter(|(_, w)| *w > 0.0);
    /// }
    ///
    /// // For just one range's vertices, index the slice it owns instead.
    /// let range = &mesh.ranges()[0];
    /// let start = range.start_vertex as usize;
    /// for v in start..start + range.vertex_count as usize {
    ///     let _ = (normals.get(v), uvs.get(v));
    /// }
    /// # let _ = tangents;
    /// ```
    pub fn vertex_buffer(&self) -> &VertexBuffer {
        &self.vertex_buffer
    }

    /// The normalized index buffer - every index is relative to the `start_vertex` of the
    /// range that owns it. See [`SkinnedMesh::range_indices`] for absolute ones.
    ///
    /// Reach for this when uploading to a GPU, where the base vertex is free; use
    /// [`IndexBuffer::as_bytes`] to hand over the `u16` data without a copy.
    pub fn index_buffer(&self) -> &IndexBuffer<u16> {
        &self.index_buffer
    }

    /// The vertex type matching this mesh's vertex buffer layout, or [`None`] if the layout is
    /// not one the `.skn` format can express.
    pub fn vertex_type(&self) -> Option<SkinnedMeshVertexType> {
        match self.vertex_buffer.description() {
            d if d == &*vertex::BASIC => Some(SkinnedMeshVertexType::Basic),
            d if d == &*vertex::COLOR => Some(SkinnedMeshVertexType::Color),
            d if d == &*vertex::TANGENT => Some(SkinnedMeshVertexType::Tangent),
            d if d == &*vertex::EXT => Some(SkinnedMeshVertexType::Ext),
            _ => None,
        }
    }

    /// The indices of a single range, resolved to absolute positions in the shared vertex
    /// buffer.
    ///
    /// The stored indices are normalized, so this adds the range's `start_vertex` back in 32
    /// bits - the same widening that lets a mesh hold more than 65536 vertices.
    ///
    /// Use this whenever the CPU touches the geometry. If you are feeding a GPU instead,
    /// prefer [`SkinnedMesh::index_buffer`] with `start_vertex` as the draw's base vertex,
    /// which costs nothing per index.
    ///
    /// ```
    /// use glam::Vec3;
    /// use ltk_mesh::{mem::vertex::ElementName, SkinnedMesh};
    /// # use ltk_mesh::{mem::{IndexBuffer, VertexBuffer, VertexBufferDescription}, SkinnedMeshRange, SkinnedMeshVertexType};
    /// # let mesh = SkinnedMesh::new(
    /// #     vec![SkinnedMeshRange::new("body", 0, 4, 0, 6)],
    /// #     VertexBuffer::new(VertexBufferDescription::from(SkinnedMeshVertexType::Basic), vec![0; 52 * 4]),
    /// #     IndexBuffer::<u16>::new([0_u16, 1, 2, 0, 2, 3].iter().flat_map(|i| i.to_le_bytes()).collect()),
    /// # );
    /// let positions = mesh.vertex_buffer().accessor::<Vec3>(ElementName::Position).unwrap();
    /// let range = &mesh.ranges()[0];
    ///
    /// let corners: Vec<Vec3> = mesh
    ///     .range_indices(range)
    ///     .map(|i| positions.get(i as usize))
    ///     .collect();
    /// assert_eq!(corners.len(), range.index_count as usize);
    /// ```
    #[must_use]
    pub fn range_indices(&self, range: &SkinnedMeshRange) -> RangeIndices<'_> {
        let (position, end) = range_index_bounds(range, self.index_buffer.count());

        RangeIndices {
            index_buffer: &self.index_buffer,
            base: u32::try_from(range.start_vertex).unwrap_or(0),
            position,
            end,
        }
    }

    fn sync_direct_blend_indices_flag(&mut self) {
        self.flags.set(
            SkinnedMeshFlags::DIRECT_BLEND_INDICES,
            self.direct_blend_index_block.is_some(),
        );
    }
}

/// Iterator over one range's indices, created by [`SkinnedMesh::range_indices`].
///
/// Yields absolute positions in the shared vertex buffer.
#[derive(Debug, Clone)]
pub struct RangeIndices<'a> {
    index_buffer: &'a IndexBuffer<u16>,
    base: u32,
    position: usize,
    end: usize,
}

impl Iterator for RangeIndices<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.position >= self.end {
            return None;
        }
        let index = self.base + u32::from(self.index_buffer.get(self.position));
        self.position += 1;
        Some(index)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.end - self.position;
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RangeIndices<'_> {}

/// The half open slice of the index buffer a range owns, clamped to what is actually there.
fn range_index_bounds(range: &SkinnedMeshRange, index_count: usize) -> (usize, usize) {
    let start = usize::try_from(range.start_index)
        .unwrap_or(0)
        .min(index_count);
    let count = usize::try_from(range.index_count).unwrap_or(0);
    (start, start.saturating_add(count).min(index_count))
}

/// Rebases every index against the `start_vertex` of the range that owns it
fn rebase_range_indices(
    index_buffer: &mut IndexBuffer<u16>,
    ranges: &[SkinnedMeshRange],
    rebase: fn(u16, u16) -> u16,
) {
    for range in ranges {
        let start_vertex = range.start_vertex as u16;
        if start_vertex == 0 {
            continue;
        }

        let (start, end) = range_index_bounds(range, index_buffer.count());
        for i in start..end {
            index_buffer.set(i, rebase(index_buffer.get(i), start_vertex));
        }
    }
}

/// Which layout a mesh's vertices use, and so how wide one vertex is.
///
/// The file stores this alongside a `vertexSize`; the two must agree or the game rejects the
/// file.
#[derive(
    TryFromPrimitive, IntoPrimitive, Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Hash,
)]
#[repr(u32)]
pub enum SkinnedMeshVertexType {
    /// 52 bytes: position, blend indices, blend weights, normal, `Texcoord0`.
    Basic,
    /// 56 bytes: [`Basic`](SkinnedMeshVertexType::Basic) plus a packed BGRA colour.
    Color,
    /// 72 bytes: [`Color`](SkinnedMeshVertexType::Color) plus a `Vec4` tangent.
    Tangent,
    /// 104 bytes: adds `Texcoord1-4`, shifting colour and tangent to +84 and +88.
    ///
    /// Added in patch 16.14. No shipped `.skn` uses it yet.
    Ext,
}

impl SkinnedMeshVertexType {
    /// The vertex stride the game requires for this type - 52, 56, 72 or 104 bytes.
    ///
    /// A file whose `vertexSize` disagrees with its `vertexType` is rejected.
    #[must_use]
    pub fn vertex_size(self) -> usize {
        VertexBufferDescription::from(self).vertex_size()
    }
}

impl From<SkinnedMeshVertexType> for VertexBufferDescription {
    fn from(value: SkinnedMeshVertexType) -> Self {
        match value {
            SkinnedMeshVertexType::Basic => vertex::BASIC.clone(),
            SkinnedMeshVertexType::Color => vertex::COLOR.clone(),
            SkinnedMeshVertexType::Tangent => vertex::TANGENT.clone(),
            SkinnedMeshVertexType::Ext => vertex::EXT.clone(),
        }
    }
}
