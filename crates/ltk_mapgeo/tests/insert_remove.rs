//! Adding and removing meshes.

use std::io::Cursor;

use glam::{Mat4, Vec3};
use ltk_mapgeo::{EditError, EnvironmentAsset, EnvironmentMesh, EnvironmentSubmesh};
use ltk_mesh::mem::{
    vertex::{VertexBufferUsage, VertexElement},
    IndexBuffer, VertexBuffer, VertexBufferDescription,
};

mod common;
use common::{asset_differences, mesh_differences};

fn read(bytes: &[u8]) -> EnvironmentAsset {
    EnvironmentAsset::from_reader(&mut Cursor::new(bytes)).unwrap()
}

fn write(asset: &EnvironmentAsset) -> Vec<u8> {
    let mut bytes = Vec::new();
    asset.to_writer(&mut bytes).unwrap();
    bytes
}

/// A version 17 map, written and read back as version 18.
fn brawl() -> EnvironmentAsset {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/maps/brawl.mapgeo");
    read(&write(&read(&std::fs::read(path).unwrap())))
}

/// A 100 x 100 quad on the ground, with a position and a UV per vertex.
fn quad() -> (Vec<VertexBuffer>, IndexBuffer<u16>) {
    let description = VertexBufferDescription::new(
        VertexBufferUsage::Static,
        vec![VertexElement::POSITION, VertexElement::TEXCOORD_0],
    );
    let corners = [(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)];
    let vertices = corners
        .iter()
        .flat_map(|&(x, z)| [x, 0.0, z, x / 100.0, z / 100.0])
        .flat_map(f32::to_le_bytes)
        .collect();
    let indices = [0u16, 1, 2, 0, 2, 3]
        .iter()
        .flat_map(|i| i.to_le_bytes())
        .collect();
    (
        vec![VertexBuffer::new(description, vertices)],
        IndexBuffer::new(indices),
    )
}

fn quad_mesh() -> EnvironmentMesh {
    EnvironmentMesh::new(vec![EnvironmentSubmesh::new(
        "Maps/Test/Materials/Quad".into(),
        0,
        6,
        0,
        3,
    )])
}

/// The vertex and index bytes that mesh `mesh` draws.
fn geometry(asset: &EnvironmentAsset, mesh: usize) -> (Vec<Vec<u8>>, Vec<u8>) {
    let m = &asset.meshes()[mesh];
    (
        m.vertex_buffer_ids()
            .iter()
            .map(|&id| asset.vertex_buffers()[id].as_bytes().to_vec())
            .collect(),
        asset.index_buffers()[m.index_buffer_id()]
            .as_bytes()
            .to_vec(),
    )
}

#[test]
fn insert_mesh_appends_mesh_that_round_trips() {
    let original = brawl();
    let mut asset = original.clone();
    let end = asset.mesh_count();
    let (vertex_buffers, index_buffer) = quad();
    let mut mesh = quad_mesh();
    mesh.set_visibility(ltk_mapgeo::EnvironmentVisibility::LAYER_1);

    asset
        .insert_mesh(end, mesh, vertex_buffers, index_buffer)
        .unwrap();
    asset
        .set_mesh_transform(end, Mat4::from_translation(Vec3::new(500.0, 10.0, 0.0)))
        .unwrap();
    let back = read(&write(&asset));

    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
    assert_eq!(back.mesh_count(), end + 1);
    let added = &back.meshes()[end];
    assert_eq!(added.bounding_box().min, Vec3::new(500.0, 10.0, 0.0));
    assert_eq!(added.bounding_box().max, Vec3::new(600.0, 10.0, 100.0));
    assert_eq!(added.submeshes()[0].material(), "Maps/Test/Materials/Quad");
    for i in 0..end {
        assert_eq!(geometry(&original, i), geometry(&back, i));
        assert_eq!(
            mesh_differences(&original.meshes()[i], &back.meshes()[i]),
            Vec::<String>::new()
        );
    }
}

#[test]
fn insert_mesh_at_front_moves_meshes_up() {
    let original = brawl();
    let mut asset = original.clone();
    let (vertex_buffers, index_buffer) = quad();

    asset
        .insert_mesh(0, quad_mesh(), vertex_buffers, index_buffer)
        .unwrap();
    let back = read(&write(&asset));

    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
    for i in 0..original.mesh_count() {
        assert_eq!(geometry(&original, i), geometry(&back, i + 1));
    }
}

#[test]
fn insert_mesh_rejects_invalid_geometry() {
    let mut asset = brawl();
    let count = asset.mesh_count();

    let (vertex_buffers, _) = quad();
    let indices = [0u16, 1, 4].iter().flat_map(|i| i.to_le_bytes()).collect();
    assert!(matches!(
        asset.insert_mesh(0, quad_mesh(), vertex_buffers, IndexBuffer::new(indices)),
        Err(EditError::IndexOutOfBounds {
            index: 4,
            vertex_count: 4
        })
    ));

    let (vertex_buffers, index_buffer) = quad();
    let mesh = EnvironmentMesh::new(vec![EnvironmentSubmesh::new(String::new(), 3, 6, 0, 3)]);
    assert!(matches!(
        asset.insert_mesh(0, mesh, vertex_buffers, index_buffer),
        Err(EditError::SubmeshOutOfBounds { submesh: 0 })
    ));

    let (mut vertex_buffers, index_buffer) = quad();
    let uv_only =
        VertexBufferDescription::new(VertexBufferUsage::Static, vec![VertexElement::TEXCOORD_0]);
    vertex_buffers[0] = VertexBuffer::new(uv_only, vec![0; 8 * 4]);
    assert!(matches!(
        asset.insert_mesh(0, quad_mesh(), vertex_buffers, index_buffer),
        Err(EditError::MissingPositions { mesh: 0 })
    ));

    let (mut vertex_buffers, index_buffer) = quad();
    let uv_only =
        VertexBufferDescription::new(VertexBufferUsage::Static, vec![VertexElement::TEXCOORD_7]);
    vertex_buffers.push(VertexBuffer::new(uv_only, vec![0; 8 * 3]));
    assert!(matches!(
        asset.insert_mesh(0, quad_mesh(), vertex_buffers, index_buffer),
        Err(EditError::VertexCountMismatch {
            buffer: 1,
            expected: 4,
            actual: 3
        })
    ));

    assert_eq!(asset.mesh_count(), count);
    assert_eq!(asset_differences(&brawl(), &asset), Vec::<String>::new());
}

#[test]
fn remove_mesh_drops_buffers_that_no_mesh_uses() {
    let original = brawl();
    let mut asset = original.clone();
    let (vertex_buffers, index_buffer) = quad();
    asset
        .insert_mesh(3, quad_mesh(), vertex_buffers, index_buffer)
        .unwrap();

    asset.remove_mesh(3);

    assert_eq!(asset_differences(&original, &asset), Vec::<String>::new());
    assert_eq!(write(&asset), write(&original));
}

#[test]
fn remove_mesh_compacts_buffer_ids() {
    let original = brawl();
    let mut asset = original.clone();

    asset.remove_mesh(0);
    let back = read(&write(&asset));

    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
    assert_eq!(back.mesh_count(), original.mesh_count() - 1);
    for i in 1..original.mesh_count() {
        assert_eq!(geometry(&original, i), geometry(&back, i - 1));
    }
}

#[test]
fn remove_mesh_keeps_buffers_that_another_mesh_uses() {
    let original = brawl();
    let mut asset = original.clone();

    let copy = asset.duplicate_mesh(0);
    asset.remove_mesh(0);

    assert_eq!(asset.vertex_buffer_count(), original.vertex_buffer_count());
    assert_eq!(asset.index_buffer_count(), original.index_buffer_count());
    assert_eq!(geometry(&original, 0), geometry(&asset, copy - 1));
}

#[test]
fn duplicate_mesh_shares_buffers() {
    let original = brawl();
    let mut asset = original.clone();

    let copy = asset.duplicate_mesh(2);
    let back = read(&write(&asset));

    assert_eq!(copy, original.mesh_count());
    assert_eq!(back.vertex_buffer_count(), original.vertex_buffer_count());
    assert_eq!(back.index_buffer_count(), original.index_buffer_count());
    assert_eq!(
        mesh_differences(&original.meshes()[2], &back.meshes()[copy]),
        Vec::<String>::new()
    );
}

#[test]
fn remove_mesh_then_insert_mesh_restores_it() {
    let original = brawl();
    let mut asset = original.clone();
    let mesh = 5;
    let vertex_buffers: Vec<_> = original.meshes()[mesh]
        .vertex_buffer_ids()
        .iter()
        .map(|&id| original.vertex_buffers()[id].clone())
        .collect();
    let index_buffer = original.index_buffers()[original.meshes()[mesh].index_buffer_id()].clone();

    let removed = asset.remove_mesh(mesh);
    asset
        .insert_mesh(mesh, removed, vertex_buffers, index_buffer)
        .unwrap();
    let back = read(&write(&asset));

    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
    assert_eq!(
        original.mesh_faces(mesh).unwrap(),
        back.mesh_faces(mesh).unwrap()
    );
    let differences = mesh_differences(&original.meshes()[mesh], &back.meshes()[mesh]);
    assert!(
        differences
            .iter()
            .all(|d| d.starts_with("vertex buffers") || d.starts_with("index buffer")),
        "{differences:?}"
    );
}
