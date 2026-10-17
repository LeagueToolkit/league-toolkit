//! Editing meshes in place.

use std::io::Cursor;

use glam::{Mat4, Vec2, Vec3};
use ltk_mapgeo::{
    EnvironmentAsset, EnvironmentAssetChannel, EnvironmentMeshRenderFlags, EnvironmentQuality,
    MeshTextureOverride, VisibilityTransitionBehavior,
};
use ltk_primitives::AABB;

mod common;
use common::asset_differences;

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

fn bits(aabb: &AABB) -> [u32; 6] {
    let [a, b, c] = aabb.min.to_array().map(f32::to_bits);
    let [d, e, f] = aabb.max.to_array().map(f32::to_bits);
    [a, b, c, d, e, f]
}

/// The first mesh with an identity transform and no region.
fn identity_mesh(asset: &EnvironmentAsset) -> usize {
    asset
        .meshes()
        .iter()
        .position(|m| *m.transform() == Mat4::IDENTITY && m.region_path_hash() == 0)
        .unwrap()
}

#[test]
fn set_mesh_transform_moves_bounding_box() {
    let mut asset = brawl();
    let mesh = identity_mesh(&asset);
    let shipped = *asset.meshes()[mesh].bounding_box();
    let offset = Vec3::new(100.0, 5.0, -250.0);

    asset
        .set_mesh_transform(mesh, Mat4::from_translation(offset))
        .unwrap();

    let moved = asset.meshes()[mesh].bounding_box();
    assert_eq!(moved.min, shipped.min + offset);
    assert_eq!(moved.max, shipped.max + offset);
}

#[test]
fn set_mesh_transform_bounds_rotated_corners() {
    let mut asset = brawl();
    let mesh = identity_mesh(&asset);
    let shipped = *asset.meshes()[mesh].bounding_box();
    let rotation = Mat4::from_rotation_y(std::f32::consts::FRAC_PI_4);

    asset.set_mesh_transform(mesh, rotation).unwrap();

    let corners = (0..8).map(|i| {
        rotation.transform_point3(Vec3::new(
            [shipped.min.x, shipped.max.x][i & 1],
            [shipped.min.y, shipped.max.y][(i >> 1) & 1],
            [shipped.min.z, shipped.max.z][(i >> 2) & 1],
        ))
    });
    assert_eq!(
        bits(asset.meshes()[mesh].bounding_box()),
        bits(&AABB::of_points(corners))
    );
}

#[test]
fn set_mesh_transform_restores_shipped_bounding_box() {
    let mut asset = brawl();
    let mesh = identity_mesh(&asset);
    let shipped = *asset.meshes()[mesh].bounding_box();

    asset
        .set_mesh_transform(mesh, Mat4::from_translation(Vec3::X))
        .unwrap();
    asset.set_mesh_transform(mesh, Mat4::IDENTITY).unwrap();

    assert_eq!(bits(asset.meshes()[mesh].bounding_box()), bits(&shipped));
}

#[test]
fn set_mesh_transform_changes_only_transform_and_bounding_box() {
    let original = brawl();
    let mut asset = original.clone();
    let mesh = identity_mesh(&asset);
    let transform = Mat4::from_translation(Vec3::new(10.0, 0.0, 20.0));

    asset.set_mesh_transform(mesh, transform).unwrap();
    let back = read(&write(&asset));

    let differences = asset_differences(&original, &back);
    assert_eq!(differences.len(), 2, "{differences:?}");
    assert!(differences[0].starts_with(&format!("mesh {mesh}: bounding box")));
    assert!(differences[1].starts_with(&format!("mesh {mesh}: transform")));
    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
}

#[test]
fn set_mesh_region_path_hash_keeps_untransformed_bounds() {
    let mut asset = brawl();
    let mesh = identity_mesh(&asset);
    let shipped = *asset.meshes()[mesh].bounding_box();

    asset
        .set_mesh_transform(mesh, Mat4::from_translation(Vec3::splat(50.0)))
        .unwrap();
    asset.set_mesh_region_path_hash(mesh, 0x0dd1_c956).unwrap();

    assert_eq!(asset.meshes()[mesh].region_path_hash(), 0x0dd1_c956);
    assert_eq!(bits(asset.meshes()[mesh].bounding_box()), bits(&shipped));
}

#[test]
fn mesh_setters_round_trip() {
    let original = brawl();
    let mut asset = original.clone();
    let mesh = &mut asset.meshes_mut()[0];

    mesh.set_quality(EnvironmentQuality::HIGH | EnvironmentQuality::VERY_HIGH);
    mesh.set_layer_transition_behavior(
        VisibilityTransitionBehavior::TurnInvisibleDoesNotMatchNewLayerFilter,
    );
    mesh.set_render_flags(
        EnvironmentMeshRenderFlags::IS_DECAL | EnvironmentMeshRenderFlags::CREATE_SHADOW_BUFFER,
    );
    mesh.set_disable_backface_culling(!mesh.disable_backface_culling());
    mesh.set_baked_light(EnvironmentAssetChannel::new(
        "ASSETS/Maps/Lightmaps/test.tex".into(),
        Vec2::new(0.5, 0.25),
        Vec2::new(0.125, 0.0),
    ));
    mesh.set_stationary_light(EnvironmentAssetChannel::new(
        "ASSETS/Maps/Lightmaps/stationary.tex".into(),
        Vec2::ONE,
        Vec2::ZERO,
    ));
    mesh.set_texture_overrides(vec![MeshTextureOverride::new(
        0,
        "ASSETS/Maps/BakedPaint/test.tex".into(),
    )]);
    mesh.set_baked_paint(EnvironmentAssetChannel::new(
        String::new(),
        Vec2::new(2.0, 2.0),
        Vec2::new(0.5, 0.5),
    ));
    mesh.submeshes_mut()[0].set_material("Maps/KitPieces/Test/Materials/Test".into());

    let back = read(&write(&asset));

    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
    let changed: Vec<_> = asset_differences(&original, &back)
        .iter()
        .map(|d| d.split(':').nth(1).unwrap().trim().to_string())
        .collect();
    assert_eq!(
        changed,
        [
            "submeshes",
            "backface culling",
            "quality",
            "transition",
            "render flags",
            "baked light",
            "stationary light",
            "texture overrides",
            "baked paint",
        ]
    );
}
