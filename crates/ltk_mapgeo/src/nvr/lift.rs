//! Lifting an `.nvr` into an [`EnvironmentAsset`]

use glam::Vec3;
use ltk_mesh::mem::{vertex::ElementName, IndexBuffer, VertexBuffer};
use ltk_primitives::AABB;

use super::{MaterialFlags, MaterialType, SimpleEnvironment};
use crate::{
    mesh::EnvironmentMeshBuilder, EnvironmentAsset, EnvironmentMesh, EnvironmentMeshRenderFlags,
    EnvironmentSubmesh, EnvironmentVisibility, FaceMask, NvrError, SceneGraphSelection,
    ShaderTextureOverride,
};

/// The shader texture overrides every shipped version 17 and 18 map carries.
const SHADER_TEXTURE_OVERRIDES: [&str; 2] =
    ["BAKED_DIFFUSE_TEXTURE", "BAKED_DIFFUSE_TEXTURE_ALPHA"];

impl SimpleEnvironment {
    /// Lifts the environment into an [`EnvironmentAsset`], which
    /// [`to_writer`](EnvironmentAsset::to_writer) writes as a version 18 `.mapgeo`.
    ///
    /// Each `.nvr` mesh becomes one [`EnvironmentMesh`] with its own vertex and index buffer and
    /// a single submesh. `material_name` names the material of that submesh from the `.nvr`
    /// material. The client resolves the name as the entry path of a `StaticMaterialDef` in the
    /// `.materials.bin` of the map.
    ///
    /// - The geometry is the [`detailed`](super::Mesh::detailed) primitive, in the layout of
    ///   [`Material::vertex_description`](super::Material::vertex_description). The
    ///   position-only [`simple`](super::Mesh::simple) primitive is dropped.
    /// - An `.nvr` stores world space positions. Every transform is identity, and the bounding
    ///   box is the bounds of the positions.
    /// - Quality is [`Mesh::quality`](super::Mesh::quality). Every mesh is on all visibility
    ///   layers and has no visibility controller.
    /// - A [`MaterialType::Decal`] mesh gets [`EnvironmentMeshRenderFlags::IS_DECAL`].
    /// - The main scene graph holds the faces of the [`MaterialFlags::GROUND`] meshes, the
    ///   terrain of the map.
    ///
    /// # Errors
    ///
    /// - [`NvrError::TooManyVertices`] if a mesh has more vertices than 16-bit indices address.
    /// - [`NvrError::IndexOutOfRange`] if a mesh index falls outside the mesh's vertex range.
    /// - [`NvrError::SceneGraph`] if baking the scene graph fails.
    pub fn to_environment_asset<F>(
        &self,
        mut material_name: F,
    ) -> Result<EnvironmentAsset, NvrError>
    where
        F: FnMut(&super::Material) -> String,
    {
        let material_names: Vec<String> = self.materials().iter().map(&mut material_name).collect();

        let mut meshes = Vec::with_capacity(self.meshes().len());
        let mut vertex_buffers = Vec::with_capacity(self.meshes().len());
        let mut index_buffers = Vec::with_capacity(self.meshes().len());
        let mut selection = SceneGraphSelection::new();

        for (index, mesh) in self.meshes().iter().enumerate() {
            let material = self.material_of(mesh);
            let vertex_count = mesh.detailed().vertex_count();
            if vertex_count > usize::from(u16::MAX) + 1 {
                return Err(NvrError::TooManyVertices {
                    mesh: index,
                    count: vertex_count,
                });
            }

            let vertices = VertexBuffer::new(
                material.vertex_description(),
                self.detailed_vertices(mesh).to_vec(),
            );
            let bounding_box = vertices
                .accessor::<Vec3>(ElementName::Position)
                .map(|positions| AABB::of_points(positions.iter()))
                .unwrap_or_default();

            let start_vertex = mesh.detailed().start_vertex() as u32;
            let mut indices = Vec::with_capacity(mesh.detailed().index_count() * 2);
            for &absolute in self.detailed_indices(mesh) {
                let local = absolute
                    .checked_sub(start_vertex)
                    .filter(|&local| (local as usize) < vertex_count)
                    .ok_or(NvrError::IndexOutOfRange {
                        mesh: index,
                        index: absolute,
                    })?;
                indices.extend_from_slice(&(local as u16).to_le_bytes());
            }
            let indices = IndexBuffer::<u16>::new(indices);

            let index_count = indices.count();
            let submesh = EnvironmentSubmesh::new(
                material_names[mesh.material()].clone(),
                0,
                index_count as i32,
                0,
                vertex_count.saturating_sub(1) as i32,
            );
            let render_flags = match material.kind() {
                MaterialType::Decal => EnvironmentMeshRenderFlags::IS_DECAL,
                _ => EnvironmentMeshRenderFlags::DEFAULT,
            };

            let faces = index_count / 3;
            selection.push_mesh(match material.flags().contains(MaterialFlags::GROUND) {
                true => FaceMask::all(faces),
                false => FaceMask::none(faces),
            });

            meshes.push(
                EnvironmentMeshBuilder::default()
                    .name(EnvironmentMesh::create_name(index))
                    .vertex_count(vertex_count as u32)
                    .vertex_buffer_ids(vec![index])
                    .index_buffer_id(index)
                    .index_count(index_count as u32)
                    .submeshes(vec![submesh])
                    .bounding_box(bounding_box)
                    .quality(mesh.quality())
                    .visibility(EnvironmentVisibility::ALL_LAYERS)
                    .render_flags(render_flags)
                    .build(),
            );
            vertex_buffers.push(vertices);
            index_buffers.push(indices);
        }

        let shader_texture_overrides = SHADER_TEXTURE_OVERRIDES
            .iter()
            .enumerate()
            .map(|(index, name)| ShaderTextureOverride::new(index as u32, (*name).to_owned()))
            .collect();

        let mut asset = EnvironmentAsset::builder()
            .shader_texture_overrides(shader_texture_overrides)
            .meshes(meshes)
            .vertex_buffers(vertex_buffers)
            .index_buffers(index_buffers)
            .build();
        let scene_graphs = asset.bake_scene_graphs(&selection)?;
        asset.replace_scene_graphs(scene_graphs);
        Ok(asset)
    }
}
