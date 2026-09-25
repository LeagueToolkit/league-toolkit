//! Helpers shared by the scene graph tests.

use ltk_mapgeo::BucketedGeometry;

/// Every difference between two grids, compared bit for bit.
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
