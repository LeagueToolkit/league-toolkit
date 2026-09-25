//! Baking scene graphs: the grid layout rules, and recovering the selection of a real map.

use std::io::Cursor;

use glam::Vec3;
use ltk_mapgeo::{
    BakeFace, BucketedGeometry, BuildError, EdgeRounding, EnvironmentAsset, EnvironmentVisibility,
    FaceMask, GridLayout, SceneGraphKey,
};

mod common;
use common::differences;

fn face(points: [(f32, f32); 3]) -> BakeFace {
    face_with(points, EnvironmentVisibility::all())
}

fn face_with(points: [(f32, f32); 3], visibility: EnvironmentVisibility) -> BakeFace {
    BakeFace::new(points.map(|(x, z)| Vec3::new(x, 0.0, z)), visibility)
}

/// Four faces on a 2x2 grid. Their min corners span (0, 0) to (100, 100), so
/// the grid runs from -10 to 110 with 60-unit buckets.
fn layout() -> BucketedGeometry {
    let faces = [
        // Min corner (0, 0): bucket (0, 0), inside.
        face([(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)]),
        // Min corner (100, 100): bucket (1, 1). x = 110 is on the upper edge,
        // so the face sticks out.
        face([(100.0, 100.0), (110.0, 100.0), (100.0, 110.0)]),
        // Min corner (40, 0): bucket (0, 0), although the centroid is in
        // bucket (1, 0). Sticks out by 50 on both axes.
        face([(40.0, 0.0), (100.0, 0.0), (100.0, 100.0)]),
        // Min corner (0, 0): bucket (0, 0), inside. Shares a vertex with the first face.
        face([(0.0, 0.0), (2.0, 1.0), (1.0, 2.0)]),
    ];
    BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(2), &faces).unwrap()
}

#[test]
fn bounds_are_padded_min_corner_bounds() {
    let grid = layout();
    assert_eq!(grid.min_bounds().to_array(), [-10.0, -10.0]);
    assert_eq!(grid.max_bounds().to_array(), [110.0, 110.0]);
    assert_eq!(grid.bucket_size().to_array(), [60.0, 60.0]);
    assert_eq!(grid.buckets_per_side(), 2);
    assert!(!grid.is_disabled());
}

#[test]
fn faces_go_to_the_bucket_of_their_min_corner() {
    let grid = layout();
    let counts: Vec<_> = grid
        .buckets()
        .iter()
        .map(|b| (b.inside_face_count(), b.sticking_out_face_count()))
        .collect();
    assert_eq!(counts, [(2, 1), (0, 0), (0, 0), (0, 1)]);
}

#[test]
fn inside_faces_come_first_in_input_order() {
    let grid = layout();
    let bucket = grid.bucket_at(0, 0).unwrap();
    let second_x: Vec<f32> = (0..bucket.total_face_count() as usize)
        .map(|f| {
            let index = grid.indices()[bucket.start_index() as usize + f * 3 + 1];
            grid.vertices()[bucket.base_vertex() as usize + index as usize].x
        })
        .collect();
    // The first, fourth and third input faces.
    assert_eq!(second_x, [10.0, 2.0, 100.0]);
}

#[test]
fn vertices_are_shared_within_a_bucket_only() {
    let grid = layout();
    // Bucket (0, 0) has 3 + 2 + 3 distinct positions. Bucket (1, 1) repeats
    // (100, 100), which bucket (0, 0) also holds.
    assert_eq!(grid.vertices().len(), 11);
    assert_eq!(&grid.indices()[..9], &[0, 1, 2, 0, 3, 4, 5, 6, 7]);
    assert_eq!(&grid.indices()[9..], &[0, 1, 2]);
}

#[test]
fn empty_buckets_keep_running_offsets() {
    let grid = layout();
    let offsets: Vec<_> = grid
        .buckets()
        .iter()
        .map(|b| (b.start_index(), b.base_vertex()))
        .collect();
    assert_eq!(offsets, [(0, 0), (9, 8), (9, 8), (9, 8)]);
}

#[test]
fn stick_out_is_the_overshoot_past_the_upper_edges() {
    let grid = layout();
    let stick_out = |x, z| grid.bucket_at(x, z).unwrap().max_stick_out().to_array();
    assert_eq!(stick_out(0, 0), [50.0, 50.0]);
    assert_eq!(stick_out(1, 1), [0.0, 0.0]);
    assert_eq!(grid.max_stick_out().to_array(), [50.0, 50.0]);
}

#[test]
fn no_faces_gives_the_shipped_empty_layout() {
    // Map22 carousel_memorybudget.mapgeo ships this grid.
    let grid = BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(128), &[]).unwrap();
    assert_eq!(grid.buckets_per_side(), 1);
    assert_eq!(
        grid.min_bounds().to_array().map(f32::to_bits),
        [0x7dcc_cccc; 2]
    );
    assert_eq!(
        grid.max_bounds().to_array().map(f32::to_bits),
        [0xfdcc_cccc; 2]
    );
    assert!(!grid.is_disabled());
    assert!(grid.vertices().is_empty() && grid.indices().is_empty());
    assert_eq!(grid.buckets().len(), 1);
    assert_eq!(grid.buckets()[0].total_face_count(), 0);
    assert!(grid.face_visibility_flags().is_none());
}

#[test]
fn face_flags_need_two_distinct_values() {
    let layer = EnvironmentVisibility::LAYER_1;
    let low = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)];
    let high = [(5.0, 5.0), (6.0, 5.0), (5.0, 6.0)];

    let same = [face_with(low, layer), face_with(high, layer)];
    let grid = BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(1), &same).unwrap();
    assert!(grid.face_visibility_flags().is_none());

    let mixed = [face_with(low, layer), face(high)];
    let grid = BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(1), &mixed).unwrap();
    assert_eq!(
        grid.face_visibility_flags(),
        Some(&[layer, EnvironmentVisibility::all()][..])
    );
}

#[test]
fn key_is_carried_over() {
    let grid = BucketedGeometry::bake(SceneGraphKey::new(1, 2), GridLayout::new(1), &[]).unwrap();
    assert_eq!(grid.visibility_controller_path_hash(), 1);
    assert_eq!(grid.region_path_hash(), 2);
}

#[test]
fn bad_input_is_rejected() {
    assert!(matches!(
        BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(0), &[]),
        Err(BuildError::ZeroBucketsPerSide)
    ));

    let faces = [
        face([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0)]),
        face([(0.0, 0.0), (f32::NAN, 0.0), (0.0, 1.0)]),
    ];
    assert!(matches!(
        BucketedGeometry::bake(SceneGraphKey::MAIN, GridLayout::new(4), &faces),
        Err(BuildError::NonFinitePosition { face: 1 })
    ));
}

fn brawl() -> EnvironmentAsset {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/maps/brawl.mapgeo");
    let data = std::fs::read(path).unwrap();
    EnvironmentAsset::from_reader(&mut Cursor::new(data)).unwrap()
}

#[test]
fn brawl_rebakes_exactly() {
    let asset = brawl();
    let selection = asset.scene_graph_selection().unwrap();
    // This map was baked with fused bucket edges, unlike the current client's maps.
    assert_eq!(
        selection.layout(SceneGraphKey::MAIN),
        GridLayout::new(128).with_edge_rounding(EdgeRounding::FusedMultiplyAdd)
    );
    let baked = asset.bake_scene_graphs(&selection).unwrap();
    assert_eq!(baked.len(), asset.scene_graphs().len());
    for (i, (b, s)) in baked.iter().zip(asset.scene_graphs()).enumerate() {
        assert_eq!(differences(b, s), Vec::<String>::new(), "scene graph {i}");
    }
}

#[test]
fn deselecting_a_mesh_changes_only_its_grid() {
    let asset = brawl();
    let mut selection = asset.scene_graph_selection().unwrap();
    let (mesh, selected) = selection
        .meshes()
        .iter()
        .enumerate()
        .find(|(_, m)| m.count() > 0)
        .map(|(i, m)| (i, m.count()))
        .unwrap();
    let key = SceneGraphKey::of_mesh(&asset.meshes()[mesh]);
    let mask = selection.mesh_mut(mesh).unwrap();
    *mask = FaceMask::none(mask.len());

    let baked = asset.bake_scene_graphs(&selection).unwrap();
    let face_count = |g: &BucketedGeometry| g.indices().len() / 3;
    for shipped in asset.scene_graphs() {
        let this = SceneGraphKey::of_graph(shipped);
        match baked.iter().find(|b| SceneGraphKey::of_graph(b) == this) {
            Some(b) if this == key => assert_eq!(face_count(b), face_count(shipped) - selected),
            Some(b) => assert!(differences(b, shipped).is_empty()),
            None => assert_eq!(this, key, "only the emptied grid may disappear"),
        }
    }
}

#[test]
fn selection_must_fit_the_asset() {
    let asset = brawl();
    let mut selection = asset.scene_graph_selection().unwrap();
    let last = selection.remove_mesh(selection.meshes().len() - 1);
    assert!(matches!(
        asset.bake_scene_graphs(&selection),
        Err(BuildError::MeshCountMismatch { .. })
    ));

    selection.push_mesh(FaceMask::all(last.len() + 1));
    assert!(matches!(
        asset.bake_scene_graphs(&selection),
        Err(BuildError::FaceMaskLength { .. })
    ));
}
