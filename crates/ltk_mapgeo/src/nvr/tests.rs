use std::io::Cursor;

use glam::{Mat4, Vec3};

use super::*;
use crate::{EnvironmentAsset, EnvironmentMeshRenderFlags, EnvironmentQuality};

const DEFAULT_STRIDE: usize = 36;

/// Builds an `.nvr` byte by byte.
#[derive(Default)]
struct Nvr(Vec<u8>);

impl Nvr {
    fn i32(&mut self, v: i32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_le_bytes());
        self
    }

    fn f32s(&mut self, vs: &[f32]) -> &mut Self {
        for v in vs {
            self.0.extend_from_slice(&v.to_le_bytes());
        }
        self
    }

    fn name(&mut self, s: &str) -> &mut Self {
        let mut buf = [0u8; NAME_LENGTH];
        buf[..s.len()].copy_from_slice(s.as_bytes());
        self.0.extend_from_slice(&buf);
        self
    }

    fn header(&mut self, version: (u16, u16), counts: [i32; 5]) -> &mut Self {
        self.0.extend_from_slice(MAGIC);
        self.0.extend_from_slice(&version.0.to_le_bytes());
        self.0.extend_from_slice(&version.1.to_le_bytes());
        for c in counts {
            self.i32(c);
        }
        self
    }

    fn channel(&mut self, texture: &str) -> &mut Self {
        self.f32s(&[1.0, 1.0, 1.0, 1.0]).name(texture);
        self.f32s(&Mat4::IDENTITY.transpose().to_cols_array())
    }

    fn material(&mut self, name: &str, kind: i32, flags: u32, texture: &str) -> &mut Self {
        self.name(name).i32(kind).u32(flags).channel(texture);
        for _ in 1..Material::CHANNEL_COUNT {
            self.channel("");
        }
        self
    }

    fn primitive(&mut self, p: [i32; 6]) -> &mut Self {
        for v in p {
            self.i32(v);
        }
        self
    }

    fn mesh(
        &mut self,
        quality: i32,
        material: i32,
        detailed: [i32; 6],
        simple: [i32; 6],
    ) -> &mut Self {
        self.i32(quality).u32(0);
        self.f32s(&[0.0; 4]).f32s(&[0.0; 6]).i32(material);
        self.primitive(detailed).primitive(simple)
    }
}

/// A 36-byte vertex at `p`, with color bytes `color`.
fn vertex(p: Vec3, color: u8) -> Vec<u8> {
    let mut out = Vec::new();
    for v in [p.x, p.y, p.z, 0.0, 1.0, 0.0, p.x / 100.0, p.z / 100.0] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out.extend_from_slice(&[color, color, color, 255]);
    out
}

/// Two quads in one vertex buffer: a ground quad at vertices 0..4 and a decal quad at 4..8,
/// the decal's indices addressing the buffer absolutely.
fn two_quads() -> Vec<u8> {
    let corners = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 0.0),
        Vec3::new(100.0, 0.0, 100.0),
        Vec3::new(0.0, 0.0, 100.0),
    ];
    let mut vertices = Vec::new();
    for (i, c) in corners.iter().enumerate() {
        vertices.extend(vertex(*c, i as u8));
    }
    for c in corners {
        vertices.extend(vertex(c + Vec3::new(500.0, 5.0, 0.0), 9));
    }
    let simple: Vec<u8> = corners
        .iter()
        .flat_map(|c| c.to_array().map(f32::to_le_bytes).concat())
        .collect();

    let detailed_indices: [u16; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];
    let simple_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    let mut nvr = Nvr::default();
    nvr.header((9, 1), [2, 2, 2, 2, 1]);
    nvr.material(
        "ground_grass_",
        0,
        MaterialFlags::GROUND.bits(),
        "grass.tga",
    );
    nvr.material("road_decal_", 1, 0, "road.tga");

    nvr.i32(vertices.len() as i32);
    nvr.0.extend(&vertices);
    nvr.i32(simple.len() as i32);
    nvr.0.extend(&simple);

    for indices in [&detailed_indices[..], &simple_indices[..]] {
        nvr.i32((indices.len() * 2) as i32).u32(INDEX_FORMAT_U16);
        for i in indices {
            nvr.0.extend_from_slice(&i.to_le_bytes());
        }
    }

    nvr.mesh(-100, 0, [0, 0, 4, 0, 0, 6], [1, 0, 4, 1, 0, 6]);
    nvr.mesh(3, 1, [0, 4, 4, 0, 6, 6], [0, 0, 0, 0, 0, 0]);

    nvr.f32s(&[0.0, 0.0, 0.0, 600.0, 5.0, 100.0])
        .i32(0)
        .i32(2)
        .i32(-1)
        .i32(0);
    nvr.0
}

fn read(bytes: &[u8]) -> Result<SimpleEnvironment, NvrError> {
    SimpleEnvironment::from_reader(&mut Cursor::new(bytes))
}

#[test]
fn reads_version_9() {
    let nvr = read(&two_quads()).unwrap();

    assert_eq!(nvr.version(), (9, 1));
    assert_eq!(nvr.materials().len(), 2);
    assert_eq!(nvr.materials()[0].name(), "ground_grass_");
    assert_eq!(nvr.materials()[0].flags(), MaterialFlags::GROUND);
    assert_eq!(nvr.materials()[0].diffuse_texture(), "grass.tga");
    assert_eq!(nvr.materials()[0].channels().len(), Material::CHANNEL_COUNT);
    assert_eq!(nvr.materials()[1].kind(), MaterialType::Decal);

    let decal = &nvr.meshes()[1];
    assert_eq!(decal.quality_level(), 3);
    assert_eq!(decal.detailed().start_vertex(), 4);
    assert_eq!(nvr.detailed_vertices(decal).len(), 4 * DEFAULT_STRIDE);
    assert_eq!(nvr.detailed_indices(decal), &[4, 5, 6, 4, 6, 7]);

    assert_eq!(nvr.nodes().len(), 1);
    assert_eq!(nvr.nodes()[0].mesh_count(), 2);
    assert_eq!(nvr.nodes()[0].first_child(), -1);
}

#[test]
fn reads_version_8_materials() {
    let mut nvr = Nvr::default();
    nvr.header((8, 1), [1, 0, 0, 0, 0]);
    nvr.name("old_").i32(0);
    nvr.f32s(&[0.5, 0.5, 0.5, 1.0]).name("diffuse.tga");
    nvr.f32s(&[0.0, 0.0, 0.0, 1.0]).name("glow.tga");

    let nvr = read(&nvr.0).unwrap();

    let material = &nvr.materials()[0];
    assert_eq!(material.flags(), MaterialFlags::empty());
    assert_eq!(material.channels().len(), 2);
    assert_eq!(material.diffuse_texture(), "diffuse.tga");
    assert_eq!(
        material.channels()[Material::EMISSIVE].texture(),
        "glow.tga"
    );
    assert_eq!(*material.channels()[0].transform(), Mat4::IDENTITY);
}

#[test]
fn rejects_bad_headers() {
    assert!(matches!(
        read(b"OEGM\x12\0\0\0"),
        Err(NvrError::InvalidFileSignature)
    ));

    let mut nvr = Nvr::default();
    nvr.header((7, 0), [0; 5]);
    assert!(matches!(
        read(&nvr.0),
        Err(NvrError::UnsupportedVersion { major: 7, minor: 0 })
    ));
}

#[test]
fn rejects_a_range_past_its_buffer() {
    let mut bytes = two_quads();
    // The decal's detailed vertex count, in the last mesh record before the node.
    let mesh_end = bytes.len() - 40;
    let vertex_count_at = mesh_end - 48 + 8;
    bytes[vertex_count_at..vertex_count_at + 4].copy_from_slice(&5i32.to_le_bytes());

    assert!(matches!(
        read(&bytes),
        Err(NvrError::RangeOutOfBounds {
            mesh: 1,
            primitive: 0
        })
    ));
}

#[test]
fn quality_level_draws_that_level_and_up() {
    let nvr = read(&two_quads()).unwrap();

    assert_eq!(nvr.meshes()[0].quality(), EnvironmentQuality::ALL);
    assert_eq!(
        nvr.meshes()[1].quality(),
        EnvironmentQuality::HIGH | EnvironmentQuality::VERY_HIGH
    );
}

#[test]
fn lifts_into_an_environment_asset() {
    let nvr = read(&two_quads()).unwrap();

    let asset = nvr
        .to_environment_asset(|m| format!("Maps/Test/{}", m.name()))
        .unwrap();

    assert_eq!(asset.meshes().len(), 2);
    let decal = &asset.meshes()[1];
    assert_eq!(decal.vertex_count(), 4);
    assert_eq!(decal.submeshes()[0].material(), "Maps/Test/road_decal_");
    assert_eq!(decal.submeshes()[0].max_vertex(), 3);
    assert_eq!(decal.render_flags(), EnvironmentMeshRenderFlags::IS_DECAL);
    assert_eq!(decal.bounding_box().min, Vec3::new(500.0, 5.0, 0.0));
    assert_eq!(decal.bounding_box().max, Vec3::new(600.0, 5.0, 100.0));

    let indices: Vec<u16> = asset.index_buffers()[1].iter().collect();
    assert_eq!(indices, [0, 1, 2, 0, 2, 3]);

    // Only the ground quad feeds the main grid.
    assert_eq!(asset.scene_graphs().len(), 1);
    assert_eq!(asset.scene_graphs()[0].indices().len(), 6);
}

#[test]
fn lifted_asset_writes_and_reads_back() {
    let nvr = read(&two_quads()).unwrap();
    let asset = nvr.to_environment_asset(|m| m.name().to_owned()).unwrap();

    let mut bytes = Vec::new();
    asset.to_writer(&mut bytes).unwrap();
    let back = EnvironmentAsset::from_reader(&mut Cursor::new(&bytes)).unwrap();

    assert_eq!(back.meshes().len(), 2);
    for (a, b) in asset.vertex_buffers().iter().zip(back.vertex_buffers()) {
        assert_eq!(a.as_bytes(), b.as_bytes());
        assert_eq!(a.description(), b.description());
    }
    assert_eq!(back.meshes()[1].quality(), asset.meshes()[1].quality());
    assert_eq!(back.shader_texture_overrides().len(), 2);
    assert_eq!(
        back.scene_graphs()[0].indices(),
        asset.scene_graphs()[0].indices()
    );
}

#[test]
fn translate_moves_every_position_once() {
    let mut nvr = read(&two_quads()).unwrap();
    let offset = Vec3::new(420.0, 7.0, 240.0);
    nvr.translate(offset);

    let asset = nvr.to_environment_asset(|m| m.name().to_owned()).unwrap();
    let decal = &asset.meshes()[1];
    assert_eq!(decal.bounding_box().min, Vec3::new(920.0, 12.0, 240.0));
    assert_eq!(decal.bounding_box().max, Vec3::new(1020.0, 12.0, 340.0));
    assert_eq!(nvr.meshes()[0].bounding_box().max, offset);
    assert_eq!(nvr.nodes()[0].bounding_box().min, offset);

    // The ground mesh's position-only primitive, in its own buffer.
    let simple = &nvr.vertex_buffers()[1];
    let x = f32::from_le_bytes(simple[12..16].try_into().unwrap());
    assert_eq!(x, 100.0 + 420.0);

    // Bytes past the position, the normal here, stay as they were.
    let normal_y = f32::from_le_bytes(nvr.vertex_buffers()[0][16..20].try_into().unwrap());
    assert_eq!(normal_y, 1.0);
}
