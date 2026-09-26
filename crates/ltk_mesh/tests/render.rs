//! Round trip and conformance tests for the `.gmesh` / `.tmesh` reader and writer.
use std::io::Cursor;

use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use glam::{Vec2, Vec3, Vec4};
use ltk_mesh::{
    error::ParseError,
    mem::{
        vertex::{ElementFormat, ElementName},
        IndexBuffer, VertexBuffer, VertexBufferDescription, VertexBufferUsage, VertexElement,
    },
    RenderMesh, RenderMeshSubmesh, GMESH_MAGIC,
};
use ltk_primitives::AABB;

const CUBE_MESH: &[u8] = include_bytes!("fixtures/hol26_mapaccents_z_cubemesh_01.gmesh");

/// Byte offset of the first vertex buffer description: magic, version, two counts, the
/// bounding box and the stream count.
const DESCRIPTIONS_OFFSET: usize = 4 + 4 + 4 + 4 + 24 + 4;

fn read(bytes: &[u8]) -> ltk_mesh::Result<RenderMesh> {
    RenderMesh::from_reader(&mut Cursor::new(bytes))
}

fn write(mesh: &RenderMesh) -> Vec<u8> {
    let mut bytes = Vec::new();
    mesh.to_writer(&mut bytes).unwrap();
    bytes
}

/// A three vertex triangle: positions in one stream, UVs in another.
fn triangle() -> RenderMesh {
    let positions = VertexBuffer::new(
        VertexBufferDescription::new(VertexBufferUsage::Static, vec![VertexElement::POSITION]),
        [0.0_f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect(),
    );
    let uvs = VertexBuffer::new(
        VertexBufferDescription::new(VertexBufferUsage::Static, vec![VertexElement::TEXCOORD_0]),
        [0.0_f32, 0.0, 1.0, 0.0, 0.0, 1.0]
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect(),
    );
    let indices =
        IndexBuffer::<u16>::new([0_u16, 1, 2].iter().flat_map(|i| i.to_le_bytes()).collect());
    RenderMesh::new(
        AABB::from_corners(Vec3::ZERO, Vec3::new(1.0, 1.0, 0.0)),
        vec![positions, uvs],
        indices,
        vec![RenderMeshSubmesh {
            material: "tri".to_string(),
            start_index: 0,
            index_count: 3,
            min_vertex: 0,
            max_vertex: 2,
        }],
    )
    .unwrap()
}

#[test]
fn shipped_gmesh_reads_the_attested_header_streams_and_submesh() {
    let mesh = read(CUBE_MESH).unwrap();

    assert_eq!(mesh.magic(), GMESH_MAGIC);
    assert_eq!(mesh.vertex_count(), 220);
    assert_eq!(mesh.index_buffer().count(), 636);
    let aabb = mesh.bounding_box();
    assert!(
        (aabb.min - Vec3::new(-249.098, -246.785, -33.899))
            .abs()
            .max_element()
            < 1e-3
    );
    assert!(
        (aabb.max - Vec3::new(249.098, 246.785, 33.899))
            .abs()
            .max_element()
            < 1e-3
    );

    let [positions, attributes] = mesh.vertex_buffers() else {
        panic!("expected two streams, got {}", mesh.vertex_buffers().len());
    };
    assert_eq!(positions.description().usage(), VertexBufferUsage::Static);
    assert_eq!(
        positions.description().elements(),
        [VertexElement::POSITION]
    );
    assert_eq!(positions.stride(), 12);
    assert_eq!(
        attributes.description().elements(),
        [
            VertexElement::new(ElementName::Normal, ElementFormat::XYZW_Float16),
            VertexElement::new(ElementName::Texcoord0, ElementFormat::XY_Float16),
            VertexElement::new(ElementName::Texcoord6, ElementFormat::XYZW_Float16),
            VertexElement::new(ElementName::Texcoord7, ElementFormat::XY_Float16),
        ]
    );
    assert_eq!(attributes.stride(), 24);

    assert_eq!(
        mesh.submeshes(),
        [RenderMeshSubmesh {
            material: "lambert1".to_string(),
            start_index: 0,
            index_count: 636,
            min_vertex: 0,
            max_vertex: 219,
        }]
    );
    assert_eq!(mesh.index_buffer().iter().min(), Some(0));
    assert_eq!(mesh.index_buffer().iter().max(), Some(219));
}

#[test]
fn shipped_gmesh_tangent_in_texcoord6_carries_a_unit_handedness_sign() {
    let mesh = read(CUBE_MESH).unwrap();
    let tangents = mesh.accessor::<Vec4>(ElementName::Texcoord6).unwrap();
    assert!(tangents.iter().all(|t| t.w.abs() == 1.0));
}

#[test]
fn accessor_finds_an_element_in_any_stream() {
    let mesh = read(CUBE_MESH).unwrap();
    assert_eq!(
        mesh.accessor::<Vec3>(ElementName::Position).unwrap().len(),
        220
    );
    assert_eq!(
        mesh.accessor::<Vec2>(ElementName::Texcoord7).unwrap().len(),
        220
    );
    assert!(mesh.accessor::<Vec2>(ElementName::PrimaryColor).is_none());
}

#[test]
fn shipped_gmesh_round_trips_byte_for_byte() {
    assert_eq!(write(&read(CUBE_MESH).unwrap()), CUBE_MESH);
}

#[test]
fn built_mesh_round_trips_through_the_writer() {
    let mesh = triangle();
    let bytes = write(&mesh);
    assert_eq!(read(&bytes).unwrap(), mesh);
}

#[test]
fn writer_pads_unused_description_slots_with_the_default_element() {
    let bytes = write(&triangle());
    let mut slots = Cursor::new(&bytes[DESCRIPTIONS_OFFSET + 8 + 8..DESCRIPTIONS_OFFSET + 128]);
    for _ in 1..VertexBufferDescription::SERIALIZED_ELEMENT_SLOTS {
        assert_eq!(
            slots.read_u32::<LE>().unwrap(),
            ElementName::Position as u32
        );
        assert_eq!(
            slots.read_u32::<LE>().unwrap(),
            ElementFormat::XYZW_Float32 as u32
        );
    }
}

#[test]
fn reader_keeps_a_magic_other_than_gmsh() {
    let mesh = triangle().with_magic(*b"TMSH");
    let bytes = write(&mesh);
    assert_eq!(&bytes[..4], b"TMSH");
    assert_eq!(read(&bytes).unwrap().magic(), *b"TMSH");
}

#[test]
fn reader_rejects_a_version_other_than_1() {
    let mut bytes = CUBE_MESH.to_vec();
    (&mut bytes[4..8]).write_u32::<LE>(2).unwrap();
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidField("version", v)) if v == "2"
    ));
}

#[test]
fn reader_rejects_a_vertex_buffer_size_the_vertex_count_disagrees_with() {
    let mut bytes = CUBE_MESH.to_vec();
    // The header vertex count is one less than the vertex count of the streams.
    (&mut bytes[8..12]).write_u32::<LE>(219).unwrap();
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidField("vertex buffer size", _))
    ));
}

#[test]
fn reader_rejects_an_element_name_in_two_streams() {
    let mut bytes = CUBE_MESH.to_vec();
    // Sets the first element of the second stream to Position.
    let second = DESCRIPTIONS_OFFSET + 128 + 8;
    (&mut bytes[second..second + 8])
        .write_u32::<LE>(ElementName::Position as u32)
        .unwrap();
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidField("vertex element name", _))
    ));
}

#[test]
fn reader_rejects_an_undefined_element_format() {
    let mut bytes = CUBE_MESH.to_vec();
    let format = DESCRIPTIONS_OFFSET + 8 + 4;
    (&mut bytes[format..format + 4]).write_u32::<LE>(9).unwrap();
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidField("vertex element format", v)) if v == "9"
    ));
}

#[test]
fn reader_rejects_a_truncated_file() {
    assert!(matches!(
        read(&CUBE_MESH[..CUBE_MESH.len() - 1]),
        Err(ParseError::IOError(_))
    ));
}

#[test]
fn new_rejects_streams_of_different_lengths() {
    let mesh = triangle();
    let mut buffers = mesh.vertex_buffers().to_vec();
    buffers[1] = VertexBuffer::new(buffers[1].description().clone(), vec![0; 8]);
    assert!(matches!(
        RenderMesh::new(
            mesh.bounding_box(),
            buffers,
            mesh.index_buffer().clone(),
            vec![]
        ),
        Err(ParseError::InvalidField("vertex stream length", _))
    ));
}

#[test]
fn new_rejects_a_mesh_without_streams() {
    assert!(matches!(
        RenderMesh::new(AABB::default(), vec![], IndexBuffer::new(vec![]), vec![]),
        Err(ParseError::InvalidField("vertex stream count", _))
    ));
}
