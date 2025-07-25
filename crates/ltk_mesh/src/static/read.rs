use crate::{error::ParseError, r#static::MAGIC, StaticMesh, StaticMeshFace};
use byteorder::{ReadBytesExt, LE};
use glam::Vec3;
use io_ext::ReaderExt;
use ltk_primitives::Color;
use std::io::Read;

impl StaticMesh {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::Result<Self> {
        let mut buf: [u8; 8] = [0; 8];
        reader.read_exact(&mut buf)?;
        if MAGIC != buf {
            return Err(ParseError::InvalidFileSignature);
        }

        let major = reader.read_u16::<LE>()?;
        let minor = reader.read_u16::<LE>()?;

        // there are versions [2][1] and [1][1] as well
        if major != 2 && major != 3 && minor != 1 {
            return Err(ParseError::InvalidFileVersion(major, minor));
        }

        let name = reader.read_padded_string::<LE, 128>()?;

        let vertex_count = reader.read_i32::<LE>()?;
        let face_count = reader.read_i32::<LE>()?;

        let _flags = reader.read_u32::<LE>()?; // TODO (alan): handle StaticMeshFlags
        let _bounding_box = reader.read_aabb::<LE>()?;

        let has_vertex_colors = match (major, minor) {
            (3.., 2..) => reader.read_i32::<LE>()? == 1,
            _ => false,
        };

        // TODO (alan): try some byte reinterp here
        let mut vertices: Vec<Vec3> = Vec::with_capacity(vertex_count as usize);
        for _ in 0..vertex_count {
            vertices.push(reader.read_vec3::<LE>()?);
        }

        let vertex_colors: Option<Vec<Color>> = match has_vertex_colors {
            true => {
                let mut v = Vec::with_capacity(vertex_count as usize);
                for _ in 0..vertex_count {
                    v.push(reader.read_color_f32::<LE>()?);
                }
                Some(v)
            }
            false => None,
        };

        let _central_point = reader.read_vec3::<LE>()?;

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
}
