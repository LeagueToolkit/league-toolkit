//! Writing `.mapgeo` files.

use std::io::Cursor;

use glam::Vec2;
use ltk_mapgeo::{
    sampler, BucketedGeometry, EnvironmentAsset, EnvironmentAssetChannel, EnvironmentVisibility,
    MeshTextureOverride, WriteError, WRITE_VERSION,
};

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

/// A version 17 map.
fn brawl() -> EnvironmentAsset {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/maps/brawl.mapgeo");
    read(&std::fs::read(path).unwrap())
}

#[test]
fn older_versions_are_written_as_the_newest() {
    let asset = brawl();
    let bytes = write(&asset);
    assert_eq!(&bytes[..4], b"OEGM");
    assert_eq!(
        u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        WRITE_VERSION
    );
    assert_eq!(
        asset_differences(&asset, &read(&bytes)),
        Vec::<String>::new()
    );
}

#[test]
fn writing_what_was_read_gives_the_same_bytes() {
    let bytes = write(&brawl());
    assert!(write(&read(&bytes)) == bytes);
}

#[test]
fn a_changed_visibility_controller_is_the_only_change() {
    let original = read(&write(&brawl()));
    let mut asset = original.clone();
    let mesh = asset
        .meshes()
        .iter()
        .position(|m| m.visibility_controller_path_hash() == 0)
        .unwrap();
    asset.meshes_mut()[mesh].set_visibility_controller_path_hash(0x1234_5678);
    asset.meshes_mut()[mesh].set_visibility(EnvironmentVisibility::empty());

    let back = read(&write(&asset));
    assert_eq!(
        asset_differences(&original, &back),
        [
            format!("mesh {mesh}: visibility controller: 0 != 305419896"),
            format!(
                "mesh {mesh}: visibility: {:?} != {:?}",
                original.meshes()[mesh].visibility(),
                EnvironmentVisibility::empty()
            ),
        ]
    );
    assert_eq!(asset_differences(&asset, &back), Vec::<String>::new());
}

#[test]
fn a_disabled_scene_graph_is_rejected() {
    let mut asset = brawl();
    asset.replace_scene_graphs(vec![BucketedGeometry::empty()]);
    assert!(matches!(
        asset.to_writer(&mut Vec::new()),
        Err(WriteError::DisabledSceneGraph { index: 0 })
    ));
}

#[test]
fn baked_paint_texture_is_written_as_diffuse_override() {
    let mut asset = brawl();
    let mesh = asset
        .meshes()
        .iter()
        .position(|m| m.texture_overrides().is_empty())
        .unwrap();
    let texture = "ASSETS/Maps/BakedPaint/Maps/MapGeometry/Map12/Base/0.tex";
    let channel =
        EnvironmentAssetChannel::new(texture.into(), Vec2::new(0.5, 0.25), Vec2::new(0.125, 0.5));
    asset.meshes_mut()[mesh].set_baked_paint(channel.clone());

    let back = read(&write(&asset));

    let written = &back.meshes()[mesh];
    assert_eq!(
        written.texture_overrides(),
        [MeshTextureOverride::new(sampler::DIFFUSE, texture.into())]
    );
    assert_eq!(written.baked_paint().scale(), channel.scale());
    assert_eq!(written.baked_paint().offset(), channel.offset());
    assert_eq!(
        written.resolve_diffuse_texture("diffuse.tex"),
        asset.meshes()[mesh].resolve_diffuse_texture("diffuse.tex")
    );
}

#[test]
fn baked_paint_texture_yields_to_diffuse_override() {
    let mut asset = brawl();
    let override_texture = "ASSETS/Maps/BakedPaint/override.tex";
    let m = &mut asset.meshes_mut()[0];
    m.set_texture_overrides(vec![MeshTextureOverride::new(
        sampler::DIFFUSE,
        override_texture.into(),
    )]);
    m.set_baked_paint(EnvironmentAssetChannel::new(
        "ASSETS/Maps/BakedPaint/channel.tex".into(),
        Vec2::ONE,
        Vec2::ZERO,
    ));

    let back = read(&write(&asset));

    assert_eq!(
        back.meshes()[0].texture_overrides(),
        [MeshTextureOverride::new(
            sampler::DIFFUSE,
            override_texture.into()
        )]
    );
}

#[test]
fn rebaked_scene_graphs_are_written() {
    let mut asset = brawl();
    let selection = asset.scene_graph_selection().unwrap();
    let baked = asset.bake_scene_graphs(&selection).unwrap();
    asset.replace_scene_graphs(baked);
    let back = read(&write(&asset));
    assert_eq!(asset_differences(&brawl(), &back), Vec::<String>::new());
}
