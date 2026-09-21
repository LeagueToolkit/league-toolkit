#![cfg(feature = "tangent-baking")]

use glam::{Vec3, Vec4};
use ltk_mesh::{
    error::BakeTangentsError,
    mem::{vertex::ElementName, IndexBuffer, VertexBuffer, VertexBufferDescription},
    SkinnedMesh, SkinnedMeshFlags, SkinnedMeshRange, SkinnedMeshVertexType,
};

fn mesh(kind: SkinnedMeshVertexType, mirrored: bool) -> SkinnedMesh {
    let points = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.], [1., 1., 0.]];
    let uvs = if mirrored {
        [[0., 0.], [1., 0.], [0., 1.], [0., 0.]]
    } else {
        [[0., 0.], [1., 0.], [0., 1.], [1., 1.]]
    };
    let mut bytes = Vec::new();
    for (i, (p, uv)) in points.into_iter().zip(uvs).enumerate() {
        for f in p {
            bytes.extend_from_slice(&f32::to_le_bytes(f));
        }
        bytes.extend_from_slice(&[i as u8, 2, 3, 4]);
        for f in [0.5_f32, 0.25, 0.125, 0.125, 0., 0., 2.] {
            bytes.extend_from_slice(&f.to_le_bytes());
        }
        for f in uv {
            bytes.extend_from_slice(&f32::to_le_bytes(f));
        }
        if kind == SkinnedMeshVertexType::Ext {
            for f in [0.125_f32; 8] {
                bytes.extend_from_slice(&f.to_le_bytes());
            }
        }
        if kind != SkinnedMeshVertexType::Basic {
            bytes.extend_from_slice(&[3, 7, 11, 19]);
        }
        if matches!(
            kind,
            SkinnedMeshVertexType::Tangent | SkinnedMeshVertexType::Ext
        ) {
            bytes.extend_from_slice(&[0; 16]);
        }
    }
    SkinnedMesh::new(
        vec![SkinnedMeshRange::new("body", 0, 4, 0, 6)],
        VertexBuffer::new(VertexBufferDescription::from(kind), bytes),
        indices(&[0, 1, 2, 1, 3, 2]),
    )
}

fn indices(values: &[u16]) -> IndexBuffer<u16> {
    IndexBuffer::new(values.iter().flat_map(|i| i.to_le_bytes()).collect())
}

fn assert_basis(mesh: &SkinnedMesh) {
    let tangents = mesh
        .vertex_buffer()
        .accessor::<Vec4>(ElementName::Texcoord6)
        .unwrap();
    let normals = mesh
        .vertex_buffer()
        .accessor::<Vec3>(ElementName::Normal)
        .unwrap();
    for (n, t) in normals.iter().zip(tangents.iter()) {
        assert!(t.is_finite());
        assert!((t.truncate().length() - 1.0).abs() < 1e-5);
        assert!(n.normalize().dot(t.truncate()).abs() < 1e-5);
        assert_eq!(t.w.abs(), 1.0);
    }
}

#[test]
fn promotes_layouts_preserves_attributes_and_round_trips() {
    for kind in [
        SkinnedMeshVertexType::Basic,
        SkinnedMeshVertexType::Color,
        SkinnedMeshVertexType::Tangent,
        SkinnedMeshVertexType::Ext,
    ] {
        let mut mesh = mesh(kind, false);
        mesh.set_flags(SkinnedMeshFlags::from_bits_retain(0x80));
        mesh.set_direct_blend_index_block(Some(vec![1, 2, 3]));
        // Preserve a nonzero opaque tail read from disk, too.
        let mut file = Vec::new();
        mesh.to_writer(&mut file).unwrap();
        let len = file.len();
        file[len - 12..].fill(42);
        let mut mesh = SkinnedMesh::from_reader(&mut file.as_slice()).unwrap();
        let before = mesh.clone();
        mesh.bake_tangents().unwrap();
        assert_basis(&mesh);
        assert_eq!(
            mesh.vertex_type(),
            Some(if kind == SkinnedMeshVertexType::Ext {
                SkinnedMeshVertexType::Ext
            } else {
                SkinnedMeshVertexType::Tangent
            })
        );
        assert_eq!(mesh.vertex_buffer().count(), 4);
        assert_eq!(mesh.ranges(), before.ranges());
        assert_eq!(mesh.index_buffer(), before.index_buffer());
        assert_eq!(mesh.flags(), before.flags());
        assert_eq!(mesh.aabb(), before.aabb());
        assert_eq!(mesh.bounding_sphere(), before.bounding_sphere());
        assert_eq!(
            mesh.direct_blend_index_block(),
            before.direct_blend_index_block()
        );
        assert_eq!(mesh.end_tab(), &[42; 12]);
        let preserved = match kind {
            SkinnedMeshVertexType::Basic => 52,
            SkinnedMeshVertexType::Color | SkinnedMeshVertexType::Tangent => 56,
            SkinnedMeshVertexType::Ext => 88,
        };
        for (old, new) in before
            .vertex_buffer()
            .as_bytes()
            .chunks_exact(kind.vertex_size())
            .zip(
                mesh.vertex_buffer()
                    .as_bytes()
                    .chunks_exact(mesh.vertex_buffer().stride()),
            )
        {
            assert_eq!(&old[..preserved], &new[..preserved]);
            if kind == SkinnedMeshVertexType::Basic {
                assert_eq!(&new[52..56], &[255; 4]);
            }
        }
        let tangents = mesh
            .vertex_buffer()
            .accessor::<Vec4>(ElementName::Texcoord6)
            .unwrap();
        for t in tangents.iter() {
            assert_eq!(t, Vec4::new(1., 0., 0., -1.));
        }
        let mut file = Vec::new();
        mesh.to_writer(&mut file).unwrap();
        assert_eq!(
            SkinnedMesh::from_reader(&mut file.as_slice()).unwrap(),
            mesh
        );
        let baked = mesh.clone();
        mesh.bake_tangents().unwrap();
        assert_eq!(mesh, baked);
    }
}

#[test]
fn splits_mirrored_uv_corners_and_copies_skinning_data() {
    let mut mesh = mesh(SkinnedMeshVertexType::Basic, true);
    let before = mesh.clone();
    mesh.bake_tangents().unwrap();
    assert_basis(&mesh);
    assert_eq!(mesh.vertex_buffer().count(), 6);
    assert_eq!(mesh.ranges()[0].vertex_count, 6);
    let t = mesh
        .vertex_buffer()
        .accessor::<Vec4>(ElementName::Texcoord6)
        .unwrap();
    let corners: Vec<_> = mesh.range_indices(&mesh.ranges()[0]).collect();
    for &v in &corners[..3] {
        assert_eq!(t.get(v as usize).w, -1.);
    }
    for &v in &corners[3..] {
        assert_eq!(t.get(v as usize).w, 1.);
    }
    for (old, new) in [0, 1, 2, 1, 3, 2].into_iter().zip(corners) {
        assert_eq!(
            &before.vertex_buffer().as_bytes()[old * 52..old * 52 + 52],
            &mesh.vertex_buffer().as_bytes()[new as usize * 72..new as usize * 72 + 52]
        );
    }
}

#[test]
fn resolves_nonzero_bases_and_keeps_unused_vertices() {
    let source = mesh(SkinnedMeshVertexType::Basic, false);
    let bytes = source.vertex_buffer().as_bytes().repeat(3);
    let mut mesh = SkinnedMesh::new(
        vec![
            SkinnedMeshRange::new("second", 4, 4, 3, 3),
            SkinnedMeshRange::new("first", 0, 4, 0, 3),
        ],
        VertexBuffer::new(
            VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
            bytes,
        ),
        indices(&[0, 1, 2, 0, 2, 1]),
    );
    mesh.bake_tangents().unwrap();
    assert_basis(&mesh);
    assert_eq!(mesh.vertex_buffer().count(), 12);
    assert_eq!(
        mesh.range_indices(&mesh.ranges()[0]).collect::<Vec<_>>(),
        [0, 2, 1]
    );
    assert_eq!(
        mesh.range_indices(&mesh.ranges()[1]).collect::<Vec<_>>(),
        [4, 5, 6]
    );
    let mut file = Vec::new();
    mesh.to_writer(&mut file).unwrap();
    assert_eq!(
        SkinnedMesh::from_reader(&mut file.as_slice()).unwrap(),
        mesh
    );
}

#[test]
fn degenerate_triangles_have_finite_perpendicular_fallbacks() {
    let source = mesh(SkinnedMeshVertexType::Basic, false);
    for values in [[0, 0, 0], [0, 1, 2]] {
        let mut bytes = source.vertex_buffer().as_bytes().to_vec();
        for vertex in bytes.chunks_exact_mut(52) {
            vertex[44..52].fill(0);
        }
        let mut mesh = SkinnedMesh::new(
            vec![SkinnedMeshRange::new("body", 0, 4, 0, 3)],
            VertexBuffer::new(
                VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
                bytes,
            ),
            indices(&values),
        );
        mesh.bake_tangents().unwrap();
        assert_basis(&mesh);
    }
}

#[test]
fn rejects_bad_geometry_without_modification() {
    let source = mesh(SkinnedMeshVertexType::Basic, false);
    for range in [
        SkinnedMeshRange::new("bad", -1, 4, 0, 6),
        SkinnedMeshRange::new("bad", 0, 4, 0, 5),
        SkinnedMeshRange::new("bad", 0, 4, 0, 9),
        SkinnedMeshRange::new("bad", 0, 3, 0, 6),
    ] {
        let mut mesh = SkinnedMesh::new(
            vec![range],
            source.vertex_buffer().clone(),
            source.index_buffer().clone(),
        );
        let before = mesh.clone();
        assert!(matches!(
            mesh.bake_tangents(),
            Err(BakeTangentsError::InvalidGeometry(_))
        ));
        assert_eq!(mesh, before);
    }
    for (offset, value) in [(32, 0_f32), (44, f32::INFINITY)] {
        let mut bytes = source.vertex_buffer().as_bytes().to_vec();
        if offset == 32 {
            bytes[32..44].fill(0);
        } else {
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut mesh = SkinnedMesh::new(
            source.ranges().to_vec(),
            VertexBuffer::new(
                VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
                bytes,
            ),
            source.index_buffer().clone(),
        );
        let before = mesh.clone();
        assert!(mesh.bake_tangents().is_err());
        assert_eq!(mesh, before);
    }
}

#[test]
fn enables_normalized_storage_above_global_vertex_limit() {
    let source = mesh(SkinnedMeshVertexType::Basic, false);
    let bytes = source.vertex_buffer().as_bytes().repeat(16385);
    let mut mesh = SkinnedMesh::new(
        source.ranges().to_vec(),
        VertexBuffer::new(
            VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
            bytes,
        ),
        source.index_buffer().clone(),
    );
    mesh.bake_tangents().unwrap();
    assert!(mesh.stores_normalized_indices());
    let mut file = Vec::new();
    mesh.to_writer(&mut file).unwrap();
    assert_eq!(
        SkinnedMesh::from_reader(&mut file.as_slice()).unwrap(),
        mesh
    );
}

#[test]
fn split_overflow_is_atomic() {
    let source = mesh(SkinnedMeshVertexType::Basic, true);
    let mut mesh = SkinnedMesh::new(
        vec![SkinnedMeshRange::new("full", 0, 65536, 0, 6)],
        VertexBuffer::new(
            VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
            source.vertex_buffer().as_bytes().repeat(16384),
        ),
        source.index_buffer().clone(),
    );
    let before = mesh.clone();
    assert!(matches!(
        mesh.bake_tangents(),
        Err(BakeTangentsError::TooManyVertices(0))
    ));
    assert_eq!(mesh, before);
}

#[test]
fn handles_empty_mesh_and_rejects_overlapping_index_ranges() {
    let mut empty = SkinnedMesh::new(
        vec![],
        VertexBuffer::new(
            VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
            vec![],
        ),
        indices(&[]),
    );
    empty.bake_tangents().unwrap();
    assert!(empty.vertex_buffer().is_empty());
    assert_eq!(empty.vertex_type(), Some(SkinnedMeshVertexType::Tangent));

    let source = mesh(SkinnedMeshVertexType::Basic, false);
    let mut overlapping = SkinnedMesh::new(
        vec![source.ranges()[0].clone(), source.ranges()[0].clone()],
        source.vertex_buffer().clone(),
        source.index_buffer().clone(),
    );
    let before = overlapping.clone();
    assert!(matches!(
        overlapping.bake_tangents(),
        Err(BakeTangentsError::InvalidGeometry(_))
    ));
    assert_eq!(overlapping, before);
}

#[test]
fn rejects_custom_vertex_layout_without_panicking() {
    use ltk_mesh::mem::{VertexBufferUsage, VertexElement};
    let mut mesh = SkinnedMesh::new(
        vec![],
        VertexBuffer::new(
            VertexBufferDescription::new(VertexBufferUsage::Static, vec![VertexElement::POSITION]),
            vec![0; 12],
        ),
        indices(&[]),
    );
    let before = mesh.clone();
    assert!(matches!(
        mesh.bake_tangents(),
        Err(BakeTangentsError::UnsupportedLayout)
    ));
    assert_eq!(mesh, before);
}
