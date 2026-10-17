//! Editing the meshes of an environment asset

use glam::{Mat4, Vec3};
use ltk_mesh::mem::vertex::ElementName;
use ltk_primitives::AABB;

use crate::{EditError, EnvironmentAsset};

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
        let bounding_box = self.bounding_box(mesh, transform, region)?;

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
        let bounding_box = self.bounding_box(mesh, transform, hash)?;

        let m = &mut self.meshes_mut()[mesh];
        m.set_region_path_hash(hash);
        m.set_bounding_box(bounding_box);
        Ok(())
    }

    /// Returns the bounding box that a shipped file stores for mesh `mesh` with `transform`
    /// and `region_path_hash`.
    ///
    /// The box bounds the eight transformed corners of the bounds of the positions. A mesh
    /// anchored to a region keeps the bounds of the untransformed positions. Both rules
    /// reproduce every shipped bounding box bit for bit.
    fn bounding_box(
        &self,
        mesh: usize,
        transform: Mat4,
        region_path_hash: u32,
    ) -> Result<AABB, EditError> {
        let positions = self.meshes()[mesh]
            .vertex_buffer_ids()
            .iter()
            .filter_map(|&id| self.vertex_buffer(id))
            .find_map(|vb| vb.accessor::<Vec3>(ElementName::Position))
            .ok_or(EditError::MissingPositions { mesh })?;
        let local = AABB::of_points(positions.iter());
        if region_path_hash != 0 {
            return Ok(local);
        }

        let corners = (0..8).map(|i| {
            let pick = |bit: usize, min: f32, max: f32| if i & bit == 0 { min } else { max };
            transform.transform_point3(Vec3::new(
                pick(1, local.min.x, local.max.x),
                pick(2, local.min.y, local.max.y),
                pick(4, local.min.z, local.max.z),
            ))
        });
        Ok(AABB::of_points(corners))
    }
}
