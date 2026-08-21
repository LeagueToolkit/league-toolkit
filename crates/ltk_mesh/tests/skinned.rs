//! Round trip and conformance tests for the `.skn` reader and writer.
use byteorder::{WriteBytesExt, LE};
use ltk_mesh::{
    error::ParseError,
    mem::{IndexBuffer, VertexBuffer, VertexBufferDescription},
    SkinnedMesh, SkinnedMeshFlags, SkinnedMeshRange, SkinnedMeshVertexType, END_TAB_SIZE,
    MAX_VERTEX_COUNT, SKN_MAGIC,
};
use std::io::{Cursor, Write};

const BASIC_VERTEX_SIZE: usize = 52;

fn write_range(buf: &mut Vec<u8>, name: &str, range: [i32; 4]) {
    let mut padded = name.as_bytes().to_vec();
    padded.resize(64, 0);
    buf.write_all(&padded).unwrap();
    for v in range {
        buf.write_i32::<LE>(v).unwrap();
    }
}

/// A minimal v4.1 file: one range, one triangle, three zeroed basic vertices.
fn v4_bytes(flags: u32, block: Option<&[u8]>) -> Vec<u8> {
    let indices: [u16; 3] = [0, 1, 2];
    let vertex_count = 3_u32;

    let mut buf = Vec::new();
    buf.write_u32::<LE>(SKN_MAGIC).unwrap();
    buf.write_u16::<LE>(4).unwrap();
    buf.write_u16::<LE>(1).unwrap();

    buf.write_u32::<LE>(1).unwrap();
    write_range(
        &mut buf,
        "body",
        [0, vertex_count as i32, 0, indices.len() as i32],
    );

    buf.write_u32::<LE>(flags).unwrap();
    buf.write_u32::<LE>(indices.len() as u32).unwrap();
    buf.write_u32::<LE>(vertex_count).unwrap();
    buf.write_u32::<LE>(BASIC_VERTEX_SIZE as u32).unwrap();
    buf.write_u32::<LE>(SkinnedMeshVertexType::Basic.into())
        .unwrap();
    // Bounds that no recomputation from the (all zero) vertex buffer could produce.
    for f in [-1.0_f32, -2.0, -3.0, 1.0, 2.0, 3.0] {
        buf.write_f32::<LE>(f).unwrap();
    }
    for f in [0.5_f32, 0.5, 0.5, 7.25] {
        buf.write_f32::<LE>(f).unwrap();
    }

    if let Some(block) = block {
        buf.write_u16::<LE>(block.len() as u16).unwrap();
        buf.write_all(block).unwrap();
    }

    for i in indices {
        buf.write_u16::<LE>(i).unwrap();
    }
    buf.extend(std::iter::repeat_n(
        0,
        BASIC_VERTEX_SIZE * vertex_count as usize,
    ));
    buf.extend([0_u8; END_TAB_SIZE]);
    buf
}

/// A v4.1 file with two ranges, the second based at vertex 100, so the index base is
/// observable.
fn two_range_bytes(flags: u32, indices: [u16; 6]) -> Vec<u8> {
    let vertex_count = 103_u32;

    let mut buf = Vec::new();
    buf.write_u32::<LE>(SKN_MAGIC).unwrap();
    buf.write_u16::<LE>(4).unwrap();
    buf.write_u16::<LE>(1).unwrap();

    buf.write_u32::<LE>(2).unwrap();
    write_range(&mut buf, "head", [0, 3, 0, 3]);
    write_range(&mut buf, "body", [100, 3, 3, 3]);

    buf.write_u32::<LE>(flags).unwrap();
    buf.write_u32::<LE>(indices.len() as u32).unwrap();
    buf.write_u32::<LE>(vertex_count).unwrap();
    buf.write_u32::<LE>(BASIC_VERTEX_SIZE as u32).unwrap();
    buf.write_u32::<LE>(SkinnedMeshVertexType::Basic.into())
        .unwrap();
    for f in [-1.0_f32, -2.0, -3.0, 1.0, 2.0, 3.0] {
        buf.write_f32::<LE>(f).unwrap();
    }
    for f in [0.5_f32, 0.5, 0.5, 7.25] {
        buf.write_f32::<LE>(f).unwrap();
    }

    for i in indices {
        buf.write_u16::<LE>(i).unwrap();
    }
    buf.extend(std::iter::repeat_n(
        0,
        BASIC_VERTEX_SIZE * vertex_count as usize,
    ));
    buf.extend([0_u8; END_TAB_SIZE]);
    buf
}

/// A pre-v4 file: no flags word, no bounds, and no end tab below major 2.
fn legacy_bytes(major: u16) -> Vec<u8> {
    let indices: [u16; 3] = [0, 1, 2];
    let vertex_count = 3_u32;

    let mut buf = Vec::new();
    buf.write_u32::<LE>(SKN_MAGIC).unwrap();
    buf.write_u16::<LE>(major).unwrap();
    buf.write_u16::<LE>(1).unwrap();

    if major > 0 {
        buf.write_u32::<LE>(1).unwrap();
        write_range(
            &mut buf,
            "body",
            [0, vertex_count as i32, 0, indices.len() as i32],
        );
    }

    buf.write_u32::<LE>(indices.len() as u32).unwrap();
    buf.write_u32::<LE>(vertex_count).unwrap();

    for i in indices {
        buf.write_u16::<LE>(i).unwrap();
    }
    buf.extend(std::iter::repeat_n(
        0,
        BASIC_VERTEX_SIZE * vertex_count as usize,
    ));
    if major >= 2 {
        buf.extend([0_u8; END_TAB_SIZE]);
    }
    buf
}

/// A well formed v4.1 file that declares no geometry at all.
fn empty_v4_bytes() -> Vec<u8> {
    let mut buf = Vec::new();
    buf.write_u32::<LE>(SKN_MAGIC).unwrap();
    buf.write_u16::<LE>(4).unwrap();
    buf.write_u16::<LE>(1).unwrap();

    buf.write_u32::<LE>(1).unwrap();
    write_range(&mut buf, "empty", [0, 0, 0, 0]);

    buf.write_u32::<LE>(0).unwrap(); // flags
    buf.write_u32::<LE>(0).unwrap(); // index count
    buf.write_u32::<LE>(0).unwrap(); // vertex count
    buf.write_u32::<LE>(BASIC_VERTEX_SIZE as u32).unwrap();
    buf.write_u32::<LE>(SkinnedMeshVertexType::Basic.into())
        .unwrap();
    for _ in 0..10 {
        buf.write_f32::<LE>(0.0).unwrap();
    }
    buf.extend([0_u8; END_TAB_SIZE]);
    buf
}

fn read(bytes: &[u8]) -> ltk_mesh::Result<SkinnedMesh> {
    SkinnedMesh::from_reader(&mut Cursor::new(bytes))
}

fn mesh_of(ranges: Vec<SkinnedMeshRange>, indices: &[u16], vertex_count: usize) -> SkinnedMesh {
    let index_buffer =
        IndexBuffer::<u16>::new(indices.iter().flat_map(|i| i.to_le_bytes()).collect());
    let vertex_buffer = VertexBuffer::new(
        VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
        vec![0; BASIC_VERTEX_SIZE * vertex_count],
    );
    SkinnedMesh::new(ranges, vertex_buffer, index_buffer)
}

#[test]
fn accepts_every_shipped_major() {
    for major in [0, 1, 2] {
        let mesh = read(&legacy_bytes(major))
            .unwrap_or_else(|e| panic!("major {major} should parse, got {e}"));
        assert_eq!(mesh.vertex_buffer().count(), 3);
    }
    assert!(read(&v4_bytes(0, None)).is_ok());
}

#[test]
fn accepts_a_mesh_with_no_geometry() {
    // A zero length vertex buffer used to panic on the way in. A parser may not panic on
    // input it is handed, however degenerate.
    let mesh = read(&empty_v4_bytes()).expect("an empty mesh parses rather than panicking");

    assert!(mesh.vertex_buffer().is_empty());
    assert!(mesh.index_buffer().is_empty());
    assert_eq!(mesh.range_indices(&mesh.ranges()[0]).len(), 0);
}

#[test]
fn rejects_major_3_and_nonzero_minor() {
    // The game compares the whole version dword, so major 3 never appears in the accepted set.
    let mut bytes = v4_bytes(0, None);
    bytes[4..6].copy_from_slice(&3_u16.to_le_bytes());
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidFileVersion(3, 1))
    ));

    let mut bytes = v4_bytes(0, None);
    bytes[6..8].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidFileVersion(4, 2))
    ));
}

#[test]
fn v0_range_spans_the_whole_mesh() {
    let mesh = read(&legacy_bytes(0)).unwrap();
    assert_eq!(
        mesh.ranges(),
        [SkinnedMeshRange::new("", 0, 3, 0, 3)],
        "the synthesised range is unnamed and covers every vertex and index"
    );
}

#[test]
fn keeps_the_bounds_stored_in_a_v4_file() {
    let mesh = read(&v4_bytes(0, None)).unwrap();
    assert_eq!(mesh.aabb().min, glam::vec3(-1.0, -2.0, -3.0));
    assert_eq!(mesh.aabb().max, glam::vec3(1.0, 2.0, 3.0));
    assert_eq!(mesh.bounding_sphere().origin, glam::vec3(0.5, 0.5, 0.5));
    assert_eq!(mesh.bounding_sphere().radius, 7.25);
}

#[test]
fn computes_bounds_for_pre_v4_files() {
    let mesh = read(&legacy_bytes(2)).unwrap();
    assert_eq!(mesh.aabb().min, glam::Vec3::ZERO);
    assert_eq!(mesh.aabb().max, glam::Vec3::ZERO);
}

#[test]
fn consumes_the_end_tab() {
    let bytes = v4_bytes(0, None);
    let mut cursor = Cursor::new(&bytes);
    SkinnedMesh::from_reader(&mut cursor).unwrap();
    assert_eq!(
        cursor.position() as usize,
        bytes.len(),
        "the 12 byte tail must be read, or an .skn embedded in a larger stream desyncs it"
    );
}

#[test]
fn reads_the_direct_blend_index_block() {
    let block = [0xAA_u8, 0xBB, 0xCC, 0xDD, 0xEE];
    let mesh = read(&v4_bytes(1, Some(&block))).unwrap();

    assert!(mesh
        .flags()
        .contains(SkinnedMeshFlags::DIRECT_BLEND_INDICES));
    assert_eq!(mesh.direct_blend_index_block(), Some(&block[..]));
    // The block sits before the index buffer, so getting its framing wrong shifts everything
    // after it.
    assert_eq!(mesh.index_buffer().iter().collect::<Vec<_>>(), [0, 1, 2]);
}

#[test]
fn reads_normalized_indices_flag() {
    let mesh = read(&v4_bytes(2, None)).unwrap();
    assert!(mesh.stores_normalized_indices());
    assert_eq!(mesh.direct_blend_index_block(), None);
}

const ABSOLUTE: [u16; 6] = [0, 1, 2, 100, 101, 102];
const NORMALIZED: [u16; 6] = [0, 1, 2, 0, 1, 2];

#[test]
fn normalizes_absolute_indices_on_read() {
    let mesh = read(&two_range_bytes(0, ABSOLUTE)).unwrap();
    assert_eq!(
        mesh.index_buffer().iter().collect::<Vec<_>>(),
        NORMALIZED,
        "the second range's indices are rebased onto its start_vertex, as the game does"
    );
}

#[test]
fn leaves_already_normalized_indices_alone() {
    let mesh = read(&two_range_bytes(2, NORMALIZED)).unwrap();
    assert_eq!(mesh.index_buffer().iter().collect::<Vec<_>>(), NORMALIZED);
}

#[test]
fn range_indices_are_absolute_either_way() {
    for (flags, indices) in [(0, ABSOLUTE), (2, NORMALIZED)] {
        let mesh = read(&two_range_bytes(flags, indices)).unwrap();
        assert_eq!(
            mesh.range_indices(&mesh.ranges()[0]).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!(
            mesh.range_indices(&mesh.ranges()[1]).collect::<Vec<_>>(),
            [100, 101, 102],
            "flags {flags} should not change what a consumer sees"
        );
    }
}

#[test]
fn round_trips_the_on_disk_index_base() {
    // Normalizing on read and expanding on write have to be exact inverses, or every
    // load/save cycle would walk the indices.
    for (flags, indices) in [(0, ABSOLUTE), (2, NORMALIZED)] {
        let bytes = two_range_bytes(flags, indices);
        let mut written = Vec::new();
        read(&bytes).unwrap().to_writer(&mut written).unwrap();
        assert_eq!(written, bytes, "flags {flags} did not round trip");
    }
}

#[test]
fn from_absolute_indices_matches_a_parsed_file() {
    let parsed = read(&two_range_bytes(0, ABSOLUTE)).unwrap();
    let built = SkinnedMesh::from_absolute_indices(
        parsed.ranges().to_vec(),
        VertexBuffer::new(
            VertexBufferDescription::from(SkinnedMeshVertexType::Basic),
            vec![0; BASIC_VERTEX_SIZE * 103],
        ),
        IndexBuffer::<u16>::new(ABSOLUTE.iter().flat_map(|i| i.to_le_bytes()).collect()),
    );

    assert_eq!(
        built.index_buffer().iter().collect::<Vec<_>>(),
        parsed.index_buffer().iter().collect::<Vec<_>>()
    );
}

#[test]
fn preserves_unknown_flag_bits() {
    let mesh = read(&v4_bytes(0x8000_0000, None)).unwrap();
    assert_eq!(mesh.flags().bits(), 0x8000_0000);
}

#[test]
fn round_trips_flags_block_and_bounds() {
    let block = [1_u8, 2, 3, 4, 5, 6, 7];
    let bytes = v4_bytes(3, Some(&block));
    let mesh = read(&bytes).unwrap();

    let mut written = Vec::new();
    mesh.to_writer(&mut written).unwrap();
    assert_eq!(written, bytes);
}

#[test]
fn round_trips_legacy_files_as_v4() {
    let mesh = read(&legacy_bytes(1)).unwrap();
    let mut written = Vec::new();
    mesh.to_writer(&mut written).unwrap();

    let reparsed = read(&written).unwrap();
    assert_eq!(reparsed.ranges(), mesh.ranges());
    assert_eq!(reparsed.aabb(), mesh.aabb());
    assert_eq!(reparsed.flags(), SkinnedMeshFlags::empty());
}

#[test]
fn rejects_too_many_vertices_without_normalized_indices() {
    let mut bytes = v4_bytes(0, None);
    // vertexCount sits 4 bytes past flags, which follows the 80 byte range.
    let offset = 4 + 4 + 4 + 80 + 4 + 4;
    bytes[offset..offset + 4].copy_from_slice(&(MAX_VERTEX_COUNT + 1).to_le_bytes());

    assert!(
        matches!(
            read(&bytes),
            Err(ParseError::InvalidField("vertex count", _))
        ),
        "an absolute u16 index cannot reach past 65535, so the game refuses the file"
    );
}

#[test]
fn normalized_indices_lift_the_vertex_cap() {
    let mut bytes = v4_bytes(2, None);
    let offset = 4 + 4 + 4 + 80 + 4 + 4;
    bytes[offset..offset + 4].copy_from_slice(&(MAX_VERTEX_COUNT + 1).to_le_bytes());

    // The declared buffer is not actually present, so this runs out of data rather than
    // tripping the cap.
    assert!(matches!(read(&bytes), Err(ParseError::IOError(_))));
}

#[test]
fn writer_rejects_a_file_the_game_would_refuse() {
    let count = MAX_VERTEX_COUNT as usize + 1;
    let mut mesh = mesh_of(
        vec![SkinnedMeshRange::new("body", 0, count as i32, 0, 3)],
        &[0, 1, 2],
        count,
    );

    assert!(matches!(
        mesh.to_writer(&mut Vec::new()),
        Err(ParseError::InvalidField("vertex count", _))
    ));

    mesh.set_flags(SkinnedMeshFlags::NORMALIZED_INDICES);
    assert!(mesh.to_writer(&mut Vec::new()).is_ok());
}

#[test]
fn writer_rejects_an_oversized_block() {
    let mut mesh = mesh_of(
        vec![SkinnedMeshRange::new("body", 0, 3, 0, 3)],
        &[0, 1, 2],
        3,
    );
    mesh.set_direct_blend_index_block(Some(vec![0; u16::MAX as usize + 1]));

    assert!(matches!(
        mesh.to_writer(&mut Vec::new()),
        Err(ParseError::InvalidField("direct blend index block", _))
    ));
}

#[test]
fn block_presence_drives_the_flag() {
    let mut mesh = mesh_of(
        vec![SkinnedMeshRange::new("body", 0, 3, 0, 3)],
        &[0, 1, 2],
        3,
    );
    assert!(mesh.flags().is_empty());

    // set_flags cannot claim a block the mesh does not have.
    mesh.set_flags(SkinnedMeshFlags::DIRECT_BLEND_INDICES | SkinnedMeshFlags::NORMALIZED_INDICES);
    assert_eq!(mesh.flags(), SkinnedMeshFlags::NORMALIZED_INDICES);

    mesh.set_direct_blend_index_block(Some(vec![1, 2, 3]));
    assert_eq!(
        mesh.flags(),
        SkinnedMeshFlags::DIRECT_BLEND_INDICES | SkinnedMeshFlags::NORMALIZED_INDICES
    );

    mesh.set_direct_blend_index_block(None);
    assert_eq!(mesh.flags(), SkinnedMeshFlags::NORMALIZED_INDICES);
}

#[test]
fn vertex_type_sizes_match_the_loader_table() {
    assert_eq!(SkinnedMeshVertexType::Basic.vertex_size(), 52);
    assert_eq!(SkinnedMeshVertexType::Color.vertex_size(), 56);
    assert_eq!(SkinnedMeshVertexType::Tangent.vertex_size(), 72);
    assert_eq!(SkinnedMeshVertexType::Ext.vertex_size(), 104);
}

#[test]
fn rejects_a_vertex_size_that_disagrees_with_the_type() {
    let mut bytes = v4_bytes(0, None);
    let offset = 4 + 4 + 4 + 80 + 4 + 4 + 4;
    bytes[offset..offset + 4].copy_from_slice(&56_u32.to_le_bytes());

    assert!(matches!(
        read(&bytes),
        Err(ParseError::InvalidField("vertex type/size", _))
    ));
}
