//! Writing `.mapgeo` files.

use std::io::Cursor;

use ltk_mapgeo::{
    BucketedGeometry, EnvironmentAsset, EnvironmentVisibility, WriteError, WRITE_VERSION,
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
fn rebaked_scene_graphs_are_written() {
    let mut asset = brawl();
    let selection = asset.scene_graph_selection().unwrap();
    let baked = asset.bake_scene_graphs(&selection).unwrap();
    asset.replace_scene_graphs(baked);
    let back = read(&write(&asset));
    assert_eq!(asset_differences(&brawl(), &back), Vec::<String>::new());
}
