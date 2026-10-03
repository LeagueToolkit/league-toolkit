//! Legacy `.nvr` environment files ("simple environments"), and lifting them into an
//! [`EnvironmentAsset`](crate::EnvironmentAsset).
//!
//! `.nvr` is the map geometry format of the first League clients. `.mapgeo` replaced it.
//! The format is read-only: the crate reads versions 8.1 and 9.1 and writes neither.
//!
//! [`SimpleEnvironment::from_reader`] reads a file as stored.
//! [`SimpleEnvironment::to_environment_asset`] lifts it into the `.mapgeo` model.
//! [`EnvironmentAsset::to_writer`](crate::EnvironmentAsset::to_writer) writes that model as a
//! version 18 `.mapgeo`.
//!
//! # Example
//!
//! ```ignore
//! use ltk_mapgeo::nvr::SimpleEnvironment;
//! use std::fs::File;
//!
//! let nvr = SimpleEnvironment::from_reader(&mut File::open("room.nvr")?)?;
//! let asset = nvr.to_environment_asset(|material| format!("Maps/Map1/{}", material.name()))?;
//! asset.to_writer(&mut File::create("base.mapgeo")?)?;
//! ```

mod lift;
mod material;
mod mesh;

pub use material::{Channel, Material, MaterialFlags, MaterialType};
pub use mesh::{Mesh, Node, Primitive};

use std::{
    collections::HashSet,
    io::{self, Read},
};

use byteorder::{ReadBytesExt, LE};
use glam::Vec3;

use crate::NvrError;

/// Magic bytes for `.nvr` files: `NVR\0`.
pub const MAGIC: &[u8; 4] = b"NVR\0";

/// Bytes per vertex of a mesh's [`simple`](Mesh::simple) primitive: one `XYZ_Float32` position.
pub const SIMPLE_VERTEX_SIZE: usize = 12;

/// Bytes a material or texture name takes, nul padded.
const NAME_LENGTH: usize = 260;

/// `D3DFMT_INDEX16`.
const INDEX_FORMAT_U16: u32 = 0x65;

/// `D3DFMT_INDEX32`.
const INDEX_FORMAT_U32: u32 = 0x66;

/// The contents of an `.nvr` file.
///
/// [`from_reader`](Self::from_reader) checks the material, buffers and ranges of every mesh. An
/// accessor never points past what the file holds.
#[derive(Debug, Clone, PartialEq)]
pub struct SimpleEnvironment {
    version: (u16, u16),
    materials: Vec<Material>,
    vertex_buffers: Vec<Vec<u8>>,
    index_buffers: Vec<Vec<u32>>,
    meshes: Vec<Mesh>,
    nodes: Vec<Node>,
}

impl SimpleEnvironment {
    /// Reads an `.nvr` file.
    ///
    /// # Errors
    ///
    /// - [`NvrError::InvalidFileSignature`] or [`NvrError::UnsupportedVersion`] for a file that
    ///   is not a version 8.1 or 9.1 `.nvr`.
    /// - [`NvrError::UnknownMaterialType`] or [`NvrError::UnsupportedIndexFormat`] for a value
    ///   outside the known set.
    /// - [`NvrError::MissingMaterial`], [`NvrError::MissingBuffer`] or
    ///   [`NvrError::RangeOutOfBounds`] if a mesh points past what the file holds.
    /// - [`NvrError::Io`] or [`NvrError::Reader`] if reading fails.
    pub fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self, NvrError> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(NvrError::InvalidFileSignature);
        }

        let major = reader.read_u16::<LE>()?;
        let minor = reader.read_u16::<LE>()?;
        let is_v8 = match (major, minor) {
            (8, 1) => true,
            (9, 1) => false,
            _ => return Err(NvrError::UnsupportedVersion { major, minor }),
        };

        let material_count = read_count(reader)?;
        let vertex_buffer_count = read_count(reader)?;
        let index_buffer_count = read_count(reader)?;
        let mesh_count = read_count(reader)?;
        let node_count = read_count(reader)?;

        let materials = (0..material_count)
            .map(|index| match is_v8 {
                true => Material::from_reader_v8(reader, index),
                false => Material::from_reader(reader, index),
            })
            .collect::<Result<Vec<_>, _>>()?;

        let vertex_buffers = (0..vertex_buffer_count)
            .map(|_| {
                let size = read_count(reader)?;
                read_bytes(reader, size)
            })
            .collect::<Result<Vec<_>, _>>()?;

        let index_buffers = (0..index_buffer_count)
            .map(|index| {
                let size = read_count(reader)?;
                let format = reader.read_u32::<LE>()?;
                let bytes = read_bytes(reader, size)?;
                match format {
                    INDEX_FORMAT_U16 => Ok(bytes
                        .chunks_exact(2)
                        .map(|b| u16::from_le_bytes([b[0], b[1]]).into())
                        .collect()),
                    INDEX_FORMAT_U32 => Ok(bytes
                        .chunks_exact(4)
                        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect()),
                    format => Err(NvrError::UnsupportedIndexFormat { index, format }),
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let meshes = (0..mesh_count)
            .map(|index| Mesh::from_reader(reader, !is_v8, index))
            .collect::<Result<Vec<_>, _>>()?;

        let nodes = (0..node_count)
            .map(|_| Node::from_reader(reader))
            .collect::<Result<Vec<_>, _>>()?;

        let environment = Self {
            version: (major, minor),
            materials,
            vertex_buffers,
            index_buffers,
            meshes,
            nodes,
        };
        environment.validate()?;
        Ok(environment)
    }

    /// The file version as `(major, minor)`.
    #[inline]
    pub fn version(&self) -> (u16, u16) {
        self.version
    }

    /// The materials.
    #[inline]
    pub fn materials(&self) -> &[Material] {
        &self.materials
    }

    /// The vertex buffers, as raw bytes. Their layout comes from the meshes that use them.
    #[inline]
    pub fn vertex_buffers(&self) -> &[Vec<u8>] {
        &self.vertex_buffers
    }

    /// The index buffers, widened to `u32`.
    #[inline]
    pub fn index_buffers(&self) -> &[Vec<u32>] {
        &self.index_buffers
    }

    /// The meshes.
    #[inline]
    pub fn meshes(&self) -> &[Mesh] {
        &self.meshes
    }

    /// The bounding volume tree, with the root last.
    #[inline]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The material of `mesh`.
    ///
    /// # Panics
    ///
    /// If `mesh` is not a mesh of this environment and names a material it does not have.
    #[inline]
    pub fn material_of(&self, mesh: &Mesh) -> &Material {
        &self.materials[mesh.material()]
    }

    /// The vertex bytes of the [`detailed`](Mesh::detailed) primitive of `mesh`.
    ///
    /// # Panics
    ///
    /// If `mesh` is not a mesh of this environment and its range runs past a buffer.
    pub fn detailed_vertices(&self, mesh: &Mesh) -> &[u8] {
        let stride = self.material_of(mesh).vertex_description().vertex_size();
        let p = mesh.detailed();
        let start = p.start_vertex() * stride;
        &self.vertex_buffers[p.vertex_buffer()][start..start + p.vertex_count() * stride]
    }

    /// The indices of the [`detailed`](Mesh::detailed) primitive of `mesh`. An index addresses
    /// the whole vertex buffer.
    ///
    /// # Panics
    ///
    /// If `mesh` is not a mesh of this environment and its range runs past a buffer.
    pub fn detailed_indices(&self, mesh: &Mesh) -> &[u32] {
        let p = mesh.detailed();
        &self.index_buffers[p.index_buffer()][p.start_index()..p.start_index() + p.index_count()]
    }

    /// Moves the whole environment by `offset`: every vertex position of both primitives, and
    /// the bounds of the meshes and nodes.
    ///
    /// A vertex that several meshes share moves once.
    pub fn translate(&mut self, offset: Vec3) {
        let mut moved: HashSet<(usize, usize)> = HashSet::new();
        for mesh in &self.meshes {
            let stride = self.materials[mesh.material()]
                .vertex_description()
                .vertex_size();
            for (p, stride) in [
                (mesh.detailed(), stride),
                (mesh.simple(), SIMPLE_VERTEX_SIZE),
            ] {
                let Some(buffer) = self.vertex_buffers.get_mut(p.vertex_buffer()) else {
                    continue;
                };
                for vertex in p.start_vertex()..p.start_vertex() + p.vertex_count() {
                    let at = vertex * stride;
                    if moved.insert((p.vertex_buffer(), at)) {
                        translate_position(&mut buffer[at..at + 12], offset);
                    }
                }
            }
        }

        for mesh in &mut self.meshes {
            mesh.translate(offset);
        }
        for node in &mut self.nodes {
            node.translate(offset);
        }
    }

    fn validate(&self) -> Result<(), NvrError> {
        for (index, mesh) in self.meshes.iter().enumerate() {
            let material =
                self.materials
                    .get(mesh.material())
                    .ok_or(NvrError::MissingMaterial {
                        mesh: index,
                        material: mesh.material() as i32,
                    })?;
            let detailed_stride = material.vertex_description().vertex_size();
            self.check_primitive(index, 0, mesh.detailed(), detailed_stride)?;

            // An unused simple primitive may name any buffer.
            let simple = mesh.simple();
            if simple.vertex_count() != 0 || simple.index_count() != 0 {
                self.check_primitive(index, 1, simple, SIMPLE_VERTEX_SIZE)?;
            }
        }
        Ok(())
    }

    fn check_primitive(
        &self,
        mesh: usize,
        primitive: usize,
        p: &Primitive,
        stride: usize,
    ) -> Result<(), NvrError> {
        let missing = NvrError::MissingBuffer { mesh, primitive };
        let vertices = self.vertex_buffers.get(p.vertex_buffer()).ok_or(missing)?;
        let missing = NvrError::MissingBuffer { mesh, primitive };
        let indices = self.index_buffers.get(p.index_buffer()).ok_or(missing)?;

        let vertex_end = p
            .start_vertex()
            .checked_add(p.vertex_count())
            .and_then(|end| end.checked_mul(stride));
        let index_end = p.start_index().checked_add(p.index_count());
        let fits = vertex_end.is_some_and(|end| end <= vertices.len())
            && index_end.is_some_and(|end| end <= indices.len());
        if !fits {
            return Err(NvrError::RangeOutOfBounds { mesh, primitive });
        }
        Ok(())
    }
}

/// Adds `offset` to the `XYZ_Float32` position in the first 12 bytes of `bytes`.
fn translate_position(bytes: &mut [u8], offset: Vec3) {
    for (component, delta) in bytes.chunks_exact_mut(4).zip(offset.to_array()) {
        let value = f32::from_le_bytes([component[0], component[1], component[2], component[3]]);
        component.copy_from_slice(&(value + delta).to_le_bytes());
    }
}

/// Reads an `i32` count, rejecting a negative one.
fn read_count<R: Read + ?Sized>(reader: &mut R) -> Result<usize, NvrError> {
    let count = reader.read_i32::<LE>()?;
    usize::try_from(count).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("negative count {count}"),
        )
        .into()
    })
}

/// Reads `len` bytes. The buffer grows as data arrives, which bounds the allocation of a
/// corrupt length by the size of the input.
fn read_bytes<R: Read + ?Sized>(reader: &mut R, len: usize) -> Result<Vec<u8>, NvrError> {
    let mut bytes = Vec::new();
    (&mut *reader).take(len as u64).read_to_end(&mut bytes)?;
    if bytes.len() != len {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
