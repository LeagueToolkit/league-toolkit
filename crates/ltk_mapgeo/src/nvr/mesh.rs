//! `.nvr` meshes and the node tree

use std::io::Read;

use byteorder::{ReadBytesExt, LE};
use glam::Vec3;
use ltk_io_ext::ReaderExt;
use ltk_primitives::{Sphere, AABB};

use crate::{EnvironmentQuality, NvrError};

/// A draw range: a run of vertices in one vertex buffer and a run of indices in one index buffer.
///
/// An index addresses the whole vertex buffer and lies in
/// `start_vertex..start_vertex + vertex_count`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Primitive {
    vertex_buffer: usize,
    start_vertex: usize,
    vertex_count: usize,
    index_buffer: usize,
    start_index: usize,
    index_count: usize,
}

impl Primitive {
    fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self, NvrError> {
        // A negative id or count reads as usize::MAX, which validation rejects.
        let mut next = || -> Result<usize, NvrError> {
            Ok(usize::try_from(reader.read_i32::<LE>()?).unwrap_or(usize::MAX))
        };
        Ok(Self {
            vertex_buffer: next()?,
            start_vertex: next()?,
            vertex_count: next()?,
            index_buffer: next()?,
            start_index: next()?,
            index_count: next()?,
        })
    }

    /// Index of the vertex buffer.
    #[inline]
    pub fn vertex_buffer(&self) -> usize {
        self.vertex_buffer
    }

    /// First vertex of the range.
    #[inline]
    pub fn start_vertex(&self) -> usize {
        self.start_vertex
    }

    /// Number of vertices in the range.
    #[inline]
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Index of the index buffer.
    #[inline]
    pub fn index_buffer(&self) -> usize {
        self.index_buffer
    }

    /// First index of the range.
    #[inline]
    pub fn start_index(&self) -> usize {
        self.start_index
    }

    /// Number of indices in the range.
    #[inline]
    pub fn index_count(&self) -> usize {
        self.index_count
    }
}

/// An `.nvr` mesh: one material drawn over two primitives.
#[derive(Debug, Clone, PartialEq)]
pub struct Mesh {
    quality_level: i32,
    flags: u32,
    bounding_sphere: Sphere,
    bounding_box: AABB,
    material: usize,
    detailed: Primitive,
    simple: Primitive,
}

impl Mesh {
    pub(super) fn from_reader<R: Read + ?Sized>(
        reader: &mut R,
        has_flags: bool,
        index: usize,
    ) -> Result<Self, NvrError> {
        let quality_level = reader.read_i32::<LE>()?;
        let flags = if has_flags {
            reader.read_u32::<LE>()?
        } else {
            0
        };
        let bounding_sphere = reader.read_sphere::<LE>()?;
        let bounding_box = reader.read_aabb::<LE>()?;
        let raw_material = reader.read_i32::<LE>()?;
        let material = usize::try_from(raw_material).map_err(|_| NvrError::MissingMaterial {
            mesh: index,
            material: raw_material,
        })?;
        let detailed = Primitive::from_reader(reader)?;
        let simple = Primitive::from_reader(reader)?;
        Ok(Self {
            quality_level,
            flags,
            bounding_sphere,
            bounding_box,
            material,
            detailed,
            simple,
        })
    }

    pub(super) fn translate(&mut self, offset: Vec3) {
        self.bounding_sphere.origin += offset;
        self.bounding_box.min += offset;
        self.bounding_box.max += offset;
    }

    /// The lowest graphics quality level that draws the mesh, from 0 (very low) to 4
    /// (very high). Negative levels draw at every level.
    #[inline]
    pub fn quality_level(&self) -> i32 {
        self.quality_level
    }

    /// The quality levels that draw the mesh: every level from
    /// [`quality_level`](Self::quality_level) up.
    ///
    /// The mapping is a reading of the levels the known files store. No client code is known
    /// for it.
    pub fn quality(&self) -> EnvironmentQuality {
        let lowest = self.quality_level.clamp(0, 4) as u32;
        EnvironmentQuality::from_bits_truncate(EnvironmentQuality::ALL.bits() >> lowest << lowest)
    }

    /// Mesh flags. A version 8.1 file stores none, and every known 9.1 mesh stores zero.
    #[inline]
    pub fn flags(&self) -> u32 {
        self.flags
    }

    /// The bounding sphere.
    #[inline]
    pub fn bounding_sphere(&self) -> Sphere {
        self.bounding_sphere
    }

    /// The axis-aligned bounding box.
    #[inline]
    pub fn bounding_box(&self) -> &AABB {
        &self.bounding_box
    }

    /// Index of the material.
    #[inline]
    pub fn material(&self) -> usize {
        self.material
    }

    /// The full-detail geometry, in the material's
    /// [`vertex_description`](super::Material::vertex_description).
    #[inline]
    pub fn detailed(&self) -> &Primitive {
        &self.detailed
    }

    /// The position-only geometry, in [`SIMPLE_VERTEX_SIZE`](super::SIMPLE_VERTEX_SIZE) byte
    /// vertices. Several meshes share one simple vertex buffer whatever their materials.
    #[inline]
    pub fn simple(&self) -> &Primitive {
        &self.simple
    }
}

/// A node of the bounding volume tree over the meshes.
///
/// The root is the last node. A leaf names a run of meshes, and an inner node a run of child
/// nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    bounding_box: AABB,
    first_mesh: i32,
    mesh_count: i32,
    first_child: i32,
    child_count: i32,
}

impl Node {
    pub(super) fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self, NvrError> {
        Ok(Self {
            bounding_box: reader.read_aabb::<LE>()?,
            first_mesh: reader.read_i32::<LE>()?,
            mesh_count: reader.read_i32::<LE>()?,
            first_child: reader.read_i32::<LE>()?,
            child_count: reader.read_i32::<LE>()?,
        })
    }

    /// Moves the bounds by `offset`, leaving the inverted bounds of an empty node as they are.
    pub(super) fn translate(&mut self, offset: Vec3) {
        if self.bounding_box.min.cmple(self.bounding_box.max).all() {
            self.bounding_box.min += offset;
            self.bounding_box.max += offset;
        }
    }

    /// The bounding box. An empty node stores an inverted one.
    #[inline]
    pub fn bounding_box(&self) -> &AABB {
        &self.bounding_box
    }

    /// First mesh under the node.
    #[inline]
    pub fn first_mesh(&self) -> i32 {
        self.first_mesh
    }

    /// Number of meshes under the node.
    #[inline]
    pub fn mesh_count(&self) -> i32 {
        self.mesh_count
    }

    /// First child node, or `-1` for a leaf.
    #[inline]
    pub fn first_child(&self) -> i32 {
        self.first_child
    }

    /// Number of child nodes.
    #[inline]
    pub fn child_count(&self) -> i32 {
        self.child_count
    }
}
