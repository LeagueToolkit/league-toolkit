//! Helpers shared by the integration tests.

use ltk_mapgeo::{BucketedGeometry, EnvironmentAsset, EnvironmentMesh};

/// Every difference between two grids, compared bit for bit.
#[allow(dead_code)]
pub fn differences(baked: &BucketedGeometry, shipped: &BucketedGeometry) -> Vec<String> {
    let bits = |v: glam::Vec2| [v.x.to_bits(), v.y.to_bits()];
    let mut out = Vec::new();
    let mut check = |name: &str, equal: bool| {
        if !equal {
            out.push(name.to_string());
        }
    };
    check(
        "visibility_controller_path_hash",
        baked.visibility_controller_path_hash() == shipped.visibility_controller_path_hash(),
    );
    check(
        "region_path_hash",
        baked.region_path_hash() == shipped.region_path_hash(),
    );
    check(
        "min_bounds",
        bits(baked.min_bounds()) == bits(shipped.min_bounds()),
    );
    check(
        "max_bounds",
        bits(baked.max_bounds()) == bits(shipped.max_bounds()),
    );
    check(
        "max_stick_out",
        bits(baked.max_stick_out()) == bits(shipped.max_stick_out()),
    );
    check(
        "bucket_size",
        bits(baked.bucket_size()) == bits(shipped.bucket_size()),
    );
    check(
        "buckets_per_side",
        baked.buckets_per_side() == shipped.buckets_per_side(),
    );
    check("is_disabled", baked.is_disabled() == shipped.is_disabled());
    check(
        "vertices",
        baked.vertices().len() == shipped.vertices().len()
            && baked
                .vertices()
                .iter()
                .zip(shipped.vertices())
                .all(|(a, b)| a.to_array().map(f32::to_bits) == b.to_array().map(f32::to_bits)),
    );
    check("indices", baked.indices() == shipped.indices());
    check(
        "buckets",
        baked.buckets().len() == shipped.buckets().len()
            && baked.buckets().iter().zip(shipped.buckets()).all(|(a, b)| {
                bits(a.max_stick_out()) == bits(b.max_stick_out())
                    && a.start_index() == b.start_index()
                    && a.base_vertex() == b.base_vertex()
                    && a.inside_face_count() == b.inside_face_count()
                    && a.sticking_out_face_count() == b.sticking_out_face_count()
            }),
    );
    check(
        "face_visibility_flags",
        baked.face_visibility_flags() == shipped.face_visibility_flags(),
    );
    out
}

/// How mesh `b` differs from mesh `a` in the fields version 18 stores, other than the
/// vertex declaration index, which the writer derives.
#[allow(dead_code)]
pub fn mesh_differences(a: &EnvironmentMesh, b: &EnvironmentMesh) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |name: &str, x: String, y: String| {
        if x != y {
            out.push(format!("{name}: {x} != {y}"));
        }
    };
    check(
        "vertex count",
        format!("{}", a.vertex_count()),
        format!("{}", b.vertex_count()),
    );
    check(
        "vertex buffers",
        format!("{:?}", a.vertex_buffer_ids()),
        format!("{:?}", b.vertex_buffer_ids()),
    );
    check(
        "index buffer",
        format!("{}", a.index_buffer_id()),
        format!("{}", b.index_buffer_id()),
    );
    check(
        "index count",
        format!("{}", a.index_count()),
        format!("{}", b.index_count()),
    );
    check(
        "submeshes",
        format!("{:?}", a.submeshes()),
        format!("{:?}", b.submeshes()),
    );
    check(
        "visibility controller",
        format!("{}", a.visibility_controller_path_hash()),
        format!("{}", b.visibility_controller_path_hash()),
    );
    check(
        "region",
        format!("{}", a.region_path_hash()),
        format!("{}", b.region_path_hash()),
    );
    check(
        "backface culling",
        format!("{}", a.disable_backface_culling()),
        format!("{}", b.disable_backface_culling()),
    );
    check(
        "bounding box",
        format!("{:?}", a.bounding_box()),
        format!("{:?}", b.bounding_box()),
    );
    check(
        "transform",
        format!("{:?}", a.transform().to_cols_array().map(f32::to_bits)),
        format!("{:?}", b.transform().to_cols_array().map(f32::to_bits)),
    );
    check(
        "quality",
        format!("{:?}", a.quality()),
        format!("{:?}", b.quality()),
    );
    check(
        "visibility",
        format!("{:?}", a.visibility()),
        format!("{:?}", b.visibility()),
    );
    check(
        "transition",
        format!("{:?}", a.layer_transition_behavior()),
        format!("{:?}", b.layer_transition_behavior()),
    );
    check(
        "render flags",
        format!("{:?}", a.render_flags()),
        format!("{:?}", b.render_flags()),
    );
    check(
        "baked light",
        format!("{:?}", a.baked_light()),
        format!("{:?}", b.baked_light()),
    );
    check(
        "stationary light",
        format!("{:?}", a.stationary_light()),
        format!("{:?}", b.stationary_light()),
    );
    check(
        "texture overrides",
        format!("{:?}", a.texture_overrides()),
        format!("{:?}", b.texture_overrides()),
    );
    check(
        "baked paint",
        format!("{:?}", (a.baked_paint().scale(), a.baked_paint().offset())),
        format!("{:?}", (b.baked_paint().scale(), b.baked_paint().offset())),
    );
    out
}

/// How `written`, read back, differs from `asset` in what version 18 stores.
#[allow(dead_code)]
pub fn asset_differences(asset: &EnvironmentAsset, written: &EnvironmentAsset) -> Vec<String> {
    let mut out = Vec::new();
    if asset.shader_texture_overrides() != written.shader_texture_overrides() {
        out.push("shader texture overrides".into());
    }
    let bytes = |a: &EnvironmentAsset| {
        (
            a.vertex_buffers()
                .iter()
                .map(|b| (b.description().clone(), b.as_bytes().to_vec()))
                .collect::<Vec<_>>(),
            a.index_buffers()
                .iter()
                .map(|b| b.as_bytes().to_vec())
                .collect::<Vec<_>>(),
        )
    };
    if bytes(asset) != bytes(written) {
        out.push("buffers".into());
    }
    if asset.meshes().len() != written.meshes().len() {
        out.push("mesh count".into());
    }
    for (i, (a, b)) in asset.meshes().iter().zip(written.meshes()).enumerate() {
        out.extend(
            mesh_differences(a, b)
                .into_iter()
                .map(|d| format!("mesh {i}: {d}")),
        );
    }
    if asset.scene_graphs().len() != written.scene_graphs().len() {
        out.push("scene graph count".into());
    }
    for (i, (a, b)) in asset
        .scene_graphs()
        .iter()
        .zip(written.scene_graphs())
        .enumerate()
    {
        out.extend(
            differences(b, a)
                .into_iter()
                .map(|d| format!("scene graph {i}: {d}")),
        );
    }
    if asset.planar_reflectors() != written.planar_reflectors() {
        out.push("planar reflectors".into());
    }
    out
}
