//! Editing the meshes of an environment asset

use glam::{Mat4, Vec3};
use ltk_mesh::mem::{vertex::ElementName, IndexBuffer, VertexBuffer};
use ltk_primitives::AABB;

use crate::{EditError, EnvironmentAsset, EnvironmentMesh};

impl EnvironmentAsset {
    /// Sets the transform of mesh `mesh` and recomputes its bounding box.
    ///
    /// The vertices are unchanged, so meshes that share a vertex buffer keep sharing it.
    /// Re-bake the scene graphs with [`bake_scene_graphs`](Self::bake_scene_graphs) if the
    /// mesh has faces in one.
    ///
    /// # Errors
    ///
    /// [`EditError::MissingPositions`] if the mesh has no position element. The mesh is
    /// unchanged in that case.
    ///
    /// # Panics
    ///
    /// If `mesh` is out of bounds.
    pub fn set_mesh_transform(&mut self, mesh: usize, transform: Mat4) -> Result<(), EditError> {
        let region = self.meshes()[mesh].region_path_hash();
        let bounding_box = self.mesh_bounding_box(mesh, transform, region)?;

        let m = &mut self.meshes_mut()[mesh];
        m.set_transform(transform);
        m.set_bounding_box(bounding_box);
        Ok(())
    }

    /// Sets the region path hash of mesh `mesh`, `0` for none, and recomputes its bounding
    /// box.
    ///
    /// The mesh's faces belong to the scene graph of its region, so re-bake the scene graphs
    /// with [`bake_scene_graphs`](Self::bake_scene_graphs) if the mesh has faces in one.
    ///
    /// # Errors
    ///
    /// [`EditError::MissingPositions`] if the mesh has no position element. The mesh is
    /// unchanged in that case.
    ///
    /// # Panics
    ///
    /// If `mesh` is out of bounds.
    pub fn set_mesh_region_path_hash(&mut self, mesh: usize, hash: u32) -> Result<(), EditError> {
        let transform = *self.meshes()[mesh].transform();
        let bounding_box = self.mesh_bounding_box(mesh, transform, hash)?;

        let m = &mut self.meshes_mut()[mesh];
        m.set_region_path_hash(hash);
        m.set_bounding_box(bounding_box);
        Ok(())
    }

    /// Inserts `mesh` at index `index`, drawing `index_buffer` over `vertex_buffers`.
    ///
    /// The asset appends the buffers. The buffer ids, the vertex and index counts and the
    /// bounding box of `mesh` are replaced. Meshes from `index` on move up by one, so insert a
    /// mask at the same index of a [`SceneGraphSelection`](crate::SceneGraphSelection).
    ///
    /// # Errors
    ///
    /// The asset is unchanged if any of these is returned.
    ///
    /// - [`EditError::MissingPositions`] if no vertex buffer has an `XYZ_Float32` position.
    /// - [`EditError::VertexCountMismatch`] if the vertex buffers hold different counts.
    /// - [`EditError::TooManyVertices`] if the buffers hold more than 65536 vertices.
    /// - [`EditError::IndexOutOfBounds`] if an index refers past the vertices.
    /// - [`EditError::TooManySubmeshes`] if `mesh` has more than
    ///   [`EnvironmentMesh::MAX_SUBMESH_COUNT`] submeshes.
    /// - [`EditError::SubmeshOutOfBounds`] if a submesh draws outside `index_buffer`.
    ///
    /// # Panics
    ///
    /// If `index` is greater than the number of meshes.
    pub fn insert_mesh(
        &mut self,
        index: usize,
        mut mesh: EnvironmentMesh,
        vertex_buffers: Vec<VertexBuffer>,
        index_buffer: IndexBuffer<u16>,
    ) -> Result<(), EditError> {
        assert!(
            index <= self.mesh_count(),
            "mesh index {index} is out of bounds"
        );

        let vertex_count = vertex_buffers.first().map_or(0, VertexBuffer::count);
        if let Some((buffer, vb)) = vertex_buffers
            .iter()
            .enumerate()
            .find(|(_, vb)| vb.count() != vertex_count)
        {
            return Err(EditError::VertexCountMismatch {
                buffer,
                expected: vertex_count,
                actual: vb.count(),
            });
        }
        if vertex_count > usize::from(u16::MAX) + 1 {
            return Err(EditError::TooManyVertices {
                count: vertex_count,
            });
        }
        if let Some(i) = index_buffer
            .iter()
            .find(|&i| usize::from(i) >= vertex_count)
        {
            return Err(EditError::IndexOutOfBounds {
                index: i,
                vertex_count,
            });
        }

        let submeshes = mesh.submeshes();
        if submeshes.len() > EnvironmentMesh::MAX_SUBMESH_COUNT {
            return Err(EditError::TooManySubmeshes {
                count: submeshes.len(),
            });
        }
        let index_count = index_buffer.count();
        if let Some(submesh) = submeshes.iter().position(|s| {
            let (Ok(start), Ok(count)) = (
                usize::try_from(s.start_index()),
                usize::try_from(s.index_count()),
            ) else {
                return true;
            };
            start + count > index_count
        }) {
            return Err(EditError::SubmeshOutOfBounds { submesh });
        }

        let bounding_box =
            bounding_box(&vertex_buffers, *mesh.transform(), mesh.region_path_hash())
                .ok_or(EditError::MissingPositions { mesh: index })?;

        let first = self.vertex_buffers.len();
        mesh.set_buffers(
            (first..first + vertex_buffers.len()).collect(),
            vertex_count as u32,
            self.index_buffers.len(),
            index_count as u32,
        );
        mesh.set_bounding_box(bounding_box);
        self.vertex_buffers.extend(vertex_buffers);
        self.index_buffers.push(index_buffer);
        self.meshes.insert(index, mesh);
        Ok(())
    }

    /// Appends a copy of mesh `mesh` that shares its vertex and index buffers, and returns the
    /// index of the copy.
    ///
    /// Push a mask for the copy onto a [`SceneGraphSelection`](crate::SceneGraphSelection).
    ///
    /// # Panics
    ///
    /// If `mesh` is out of bounds.
    pub fn duplicate_mesh(&mut self, mesh: usize) -> usize {
        let copy = self.meshes[mesh].clone();
        self.meshes.push(copy);
        self.meshes.len() - 1
    }

    /// Removes mesh `mesh`, drops the vertex and index buffers that no remaining mesh uses,
    /// and returns the removed mesh.
    ///
    /// The buffer ids of the returned mesh refer to the buffers before the removal.
    /// [`insert_mesh`](Self::insert_mesh) replaces them. Meshes after `mesh` move down by one,
    /// so remove the mask at the same index of a
    /// [`SceneGraphSelection`](crate::SceneGraphSelection).
    ///
    /// # Panics
    ///
    /// If `mesh` is out of bounds.
    pub fn remove_mesh(&mut self, mesh: usize) -> EnvironmentMesh {
        let removed = self.meshes.remove(mesh);

        let mut vertex_used = vec![false; self.vertex_buffers.len()];
        let mut index_used = vec![false; self.index_buffers.len()];
        for m in &self.meshes {
            for &id in m.vertex_buffer_ids() {
                vertex_used[id] = true;
            }
            index_used[m.index_buffer_id()] = true;
        }
        let vertex_ids = retain_used(&mut self.vertex_buffers, &vertex_used);
        let index_ids = retain_used(&mut self.index_buffers, &index_used);

        for m in &mut self.meshes {
            for id in m.vertex_buffer_ids_mut() {
                *id = vertex_ids[*id];
            }
            m.set_index_buffer_id(index_ids[m.index_buffer_id()]);
        }
        removed
    }

    /// Returns the bounding box of mesh `mesh` with `transform` and `region_path_hash`.
    ///
    /// Returns [`EditError::MissingPositions`] if the mesh has no position element.
    fn mesh_bounding_box(
        &self,
        mesh: usize,
        transform: Mat4,
        region_path_hash: u32,
    ) -> Result<AABB, EditError> {
        let vertex_buffers = self.meshes()[mesh]
            .vertex_buffer_ids()
            .iter()
            .filter_map(|&id| self.vertex_buffer(id));
        bounding_box(vertex_buffers, transform, region_path_hash)
            .ok_or(EditError::MissingPositions { mesh })
    }
}

/// Returns the bounding box that a shipped file stores for a mesh with `vertex_buffers`,
/// `transform` and `region_path_hash`.
///
/// The box bounds the eight transformed corners of the bounds of the positions. A mesh
/// anchored to a region keeps the bounds of the untransformed positions. Both rules reproduce
/// every shipped bounding box bit for bit. Returns `None` if no buffer has a position element.
fn bounding_box<'a>(
    vertex_buffers: impl IntoIterator<Item = &'a VertexBuffer>,
    transform: Mat4,
    region_path_hash: u32,
) -> Option<AABB> {
    let positions = vertex_buffers
        .into_iter()
        .find_map(|vb| vb.accessor::<Vec3>(ElementName::Position))?;
    let local = AABB::of_points(positions.iter());
    if region_path_hash != 0 {
        return Some(local);
    }

    let corners = (0..8).map(|i| {
        let pick = |bit: usize, min: f32, max: f32| if i & bit == 0 { min } else { max };
        transform.transform_point3(Vec3::new(
            pick(1, local.min.x, local.max.x),
            pick(2, local.min.y, local.max.y),
            pick(4, local.min.z, local.max.z),
        ))
    });
    Some(AABB::of_points(corners))
}

/// Drops the buffers that are not `used`, and returns the new id of each kept buffer by its
/// old id.
fn retain_used<T>(buffers: &mut Vec<T>, used: &[bool]) -> Vec<usize> {
    let mut next = 0;
    let ids = used
        .iter()
        .map(|&u| {
            let id = next;
            next += usize::from(u);
            id
        })
        .collect();

    let mut keep = used.iter();
    buffers.retain(|_| keep.next().copied().unwrap_or(false));
    ids
}
