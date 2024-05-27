use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use log::debug;
use vecmath::Vector3;

use crate::core::primitives::Color;

mod face;
pub use face::*;
use crate::core::mesh::error::ParseError;

const MAGIC: &[u8] = "r3d2Mesh".as_bytes();

#[derive(Clone, Debug)]
pub struct StaticMesh {
    name: String,

    vertices: Vec<Vector3<f32>>,
    faces: Vec<StaticMeshFace>,
    vertex_colors: Option<Vec<Color>>,
}

// TODO (alan): figure out endianness

impl StaticMesh {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::core::mesh::Result<Self> {
        use crate::util::ReaderExt as _;
        let mut buf: [u8; 8] = [0; 8];
        reader.read_exact(&mut buf)?;
        if MAGIC != buf {
            return Err(ParseError::InvalidFileSignature);
        }

        let major = reader.read_u16::<LittleEndian>()?;
        let minor = reader.read_u16::<LittleEndian>()?;
        debug!("version: {major}.{minor}");

        // there are versions [2][1] and [1][1] as well
        if major != 2 && major != 3 && minor != 1 {
            return Err(ParseError::InvalidFileVersion(major, minor));
        }

        let name = reader.read_padded_string::<LittleEndian, 128>()?;
        debug!("name: {name}");

        let vertex_count = reader.read_i32::<LittleEndian>()?;
        let face_count = reader.read_i32::<LittleEndian>()?;

        let _flags = reader.read_u32::<LittleEndian>()?; // TODO (alan): handle StaticMeshFlags
        let _bounding_box = reader.read_bbox_f32::<LittleEndian>()?;

        let has_vertex_colors = match (major, minor) {
            (3.., 2..) => reader.read_i32::<LittleEndian>()? == 1,
            _ => false,
        };

        // TODO (alan): try some byte reinterp here
        let mut vertices: Vec<Vector3<f32>> = Vec::with_capacity(vertex_count as usize);
        for _ in 0..vertex_count {
            vertices.push(reader.read_vector3_f32::<LittleEndian>()?);
        }

        let vertex_colors: Option<Vec<Color>> = match has_vertex_colors {
            true => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for _ in 0..vertex_count {
                    v.push(reader.read_color::<LittleEndian>()?);
                }
                Some(v)
            }
            false => None,
        };

        let _central_point = reader.read_vector3_f32::<LittleEndian>()?;

        let mut faces = Vec::with_capacity(face_count as usize);
        for _ in 0..face_count {
            faces.push(StaticMeshFace::from_reader(reader)?);
        }

        // TODO (alan): read face vertex colors or something (StaticMeshFlags::HasVcp)

        Ok(Self {
            name,
            vertices,
            faces,
            vertex_colors,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn vertices(&self) -> &[[f32; 3]] {
        &self.vertices
    }

    pub fn faces(&self) -> &[StaticMeshFace] {
        &self.faces
    }

    pub fn vertex_colors(&self) -> Option<&Vec<Color>> {
        self.vertex_colors.as_ref()
    }
}
