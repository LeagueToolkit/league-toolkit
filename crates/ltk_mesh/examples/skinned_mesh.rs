//! Reads a `.skn` and walks its geometry the way a consumer should.
//!
//! Usage: `cargo run -p ltk_mesh --example skinned_mesh -- <PATH_TO_SKN>`

use std::{fs::File, io::BufReader};

use glam::{Vec2, Vec3, Vec4};
use ltk_mesh::{mem::vertex::ElementName, SkinnedMesh, SkinnedMeshFlags};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("Usage: 'skinned_mesh [PATH_TO_SKN]'");
        return Ok(());
    };

    // Buffer the file: the reader takes many small reads for the header and submesh table.
    let mesh = SkinnedMesh::from_reader(&mut BufReader::new(File::open(&path)?))?;

    let vertices = mesh.vertex_buffer();
    println!("{path}");
    println!(
        "  {} vertices ({:?}, {} B stride), {} indices, {} submeshes",
        vertices.count(),
        mesh.vertex_type()
            .expect("parsed meshes have a known layout"),
        vertices.stride(),
        mesh.index_buffer().count(),
        mesh.ranges().len(),
    );
    println!(
        "  aabb {:?} -> {:?}, sphere r={}",
        mesh.aabb().min,
        mesh.aabb().max,
        mesh.bounding_sphere().radius,
    );

    if !mesh.flags().is_empty() {
        println!("  flags {:?}", mesh.flags());
    }
    if let Some(block) = mesh.direct_blend_index_block() {
        // Opaque as of 16.15 - preserved on write, not interpreted.
        println!("  direct blend index block: {} bytes", block.len());
    }

    // Resolve every accessor once, outside the loops. Each one is a lookup, and the elements
    // beyond the first five depend on the vertex type, so they come back as Option.
    let positions = vertices
        .accessor::<Vec3>(ElementName::Position)
        .expect("every .skn carries positions");
    let normals = vertices
        .accessor::<Vec3>(ElementName::Normal)
        .expect("every .skn carries normals");
    let uvs = vertices
        .accessor::<Vec2>(ElementName::Texcoord0)
        .expect("every .skn carries a diffuse uv");
    let blend_indices = vertices
        .accessor::<[u8; 4]>(ElementName::BlendIndex)
        .expect("every .skn carries blend indices");
    let blend_weights = vertices
        .accessor::<Vec4>(ElementName::BlendWeight)
        .expect("every .skn carries blend weights");
    let colors = vertices.accessor::<[u8; 4]>(ElementName::PrimaryColor);
    let tangents = vertices.accessor::<Vec4>(ElementName::Texcoord6);

    println!(
        "  optional elements: colour {}, tangent {}",
        colors.is_some(),
        tangents.is_some(),
    );

    for range in mesh.ranges() {
        // The index buffer is normalized, so range_indices adds start_vertex back for us.
        // A GPU consumer would instead upload index_buffer().as_bytes() untouched and pass
        // range.start_vertex as the draw's base vertex.
        let mut indices = mesh.range_indices(range);
        let mut area = 0.0_f32;
        let mut triangles = 0_usize;

        while let (Some(a), Some(b), Some(c)) = (indices.next(), indices.next(), indices.next()) {
            let (a, b, c) = (
                positions.get(a as usize),
                positions.get(b as usize),
                positions.get(c as usize),
            );
            area += (b - a).cross(c - a).length() * 0.5;
            triangles += 1;
        }

        println!(
            "  '{}': {} vertices, {triangles} triangles, area {area:.1}",
            range.material, range.vertex_count,
        );
    }

    // Per-vertex passes want iter(), which walks the whole buffer in one sweep.
    let max_influences = blend_indices
        .iter()
        .zip(blend_weights.iter())
        .map(|(_, w)| w.to_array().iter().filter(|w| **w > 0.0).count())
        .max()
        .unwrap_or(0);
    println!("  up to {max_influences} influences per vertex (the format caps at 4)");

    let uv_bounds = uvs.iter().fold((Vec2::MAX, Vec2::MIN), |(min, max), uv| {
        (min.min(uv), max.max(uv))
    });
    println!("  uv0 {:?} -> {:?}", uv_bounds.0, uv_bounds.1);
    println!("  first normal {:?}", normals.get(0));

    // Writing back is byte exact for a v4 file: the indices are expanded to the on-disk base
    // again unless NORMALIZED_INDICES asks otherwise.
    let mut written = Vec::new();
    mesh.to_writer(&mut written)?;
    println!(
        "  re-encodes to {} bytes ({})",
        written.len(),
        if mesh.flags().contains(SkinnedMeshFlags::NORMALIZED_INDICES) {
            "indices kept normalized"
        } else {
            "indices expanded back to absolute"
        },
    );

    Ok(())
}
