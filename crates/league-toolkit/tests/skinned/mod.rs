use std::io::BufReader;
use glam::vec3;
use insta::assert_debug_snapshot;
use league_toolkit::core::mesh::{SkinnedMesh, SkinnedMeshRange};
use league_toolkit::core::primitives::{AABB, Sphere};

#[test]
pub fn read() {
    let mesh = SkinnedMesh::from_reader(&mut &include_bytes!("jackinthebox.skn")[..]).unwrap();

    assert_eq!(mesh.aabb(), AABB::new(vec3(-11.59685, 0.16613889, -5.102246), vec3(11.607941, 29.03124, 10.6147995)));
    assert_eq!(mesh.bounding_sphere(), Sphere::new(vec3(0.0055451393, 14.59869, 2.7562768), 20.116425));
    assert_eq!(mesh.ranges(), [SkinnedMeshRange::new("lambert2", 0, 573, 0, 2067)]);

    assert_debug_snapshot!(mesh);
}


#[test]
pub fn round_trip() {
    let mut raw = &include_bytes!("jackinthebox.skn")[..];
    let mesh = SkinnedMesh::from_reader(&mut raw).unwrap();

    let mut vec = Vec::with_capacity(raw.len());
    mesh.to_writer(&mut vec).unwrap();

    let rt_mesh = SkinnedMesh::from_reader(&mut &vec[..]).unwrap();

    assert_eq!(mesh, rt_mesh);
}