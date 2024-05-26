use std::io::{self, Read};

use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};
use log::debug;
use vecmath::{Vector2, Vector3, Vector4};

use crate::core::{
    mesh::ParseError,
    primitives::{Color, Sphere, AABB},
};

mod face;
pub use face::*;

const MAGIC: &[u8] = "r3d2Mesh".as_bytes();

#[derive(Clone, Debug)]
pub struct StaticMesh {
    name: String,

    vertices: Vec<Vector3<f32>>,
    faces: Vec<StaticMeshFace>,
    vertex_colors: Option<Vec<Color>>,
}

// TODO (alan): figure out endianness

// TODO (alan): move/rename this
pub trait ReaderExt: Read {
    // FIXME (alan): make own result type here
    fn read_padded_string<T: ByteOrder, const N: usize>(
        &mut self,
    ) -> crate::core::mesh::Result<String> {
        let mut buf: [u8; N] = [0; N];
        self.read_exact(&mut buf)?;
        let i = buf.iter().position(|&b| b == b'\0').unwrap_or(buf.len());
        Ok(std::str::from_utf8(&buf[..i])?.to_string())
    }

    fn read_color<T: ByteOrder>(&mut self) -> io::Result<Color> {
        Ok(Color {
            r: self.read_f32::<T>()?,
            g: self.read_f32::<T>()?,
            b: self.read_f32::<T>()?,
            a: self.read_f32::<T>()?,
        })
    }

    fn read_vector2_f32<T: ByteOrder>(&mut self) -> io::Result<Vector2<f32>> {
        Ok([self.read_f32::<T>()?, self.read_f32::<T>()?])
    }
    fn read_vector3_f32<T: ByteOrder>(&mut self) -> io::Result<Vector3<f32>> {
        Ok([
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ])
    }
    fn read_vector4_f32<T: ByteOrder>(&mut self) -> io::Result<Vector4<f32>> {
        Ok([
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ])
    }

    // TODO (alan): quaternion type (maybe vecmath is not the play)
    fn read_quaternion_f32<T: ByteOrder>(&mut self) -> io::Result<Vector4<f32>> {
        Ok([
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
            self.read_f32::<T>()?,
        ])
    }

    fn read_bbox_f32<T: ByteOrder>(&mut self) -> io::Result<AABB<f32>> {
        Ok(AABB {
            min: self.read_vector3_f32::<T>()?,
            max: self.read_vector3_f32::<T>()?,
        })
    }

    fn read_sphere_f32<T: ByteOrder>(&mut self) -> io::Result<Sphere> {
        Ok(Sphere::new(
            self.read_vector3_f32::<T>()?,
            self.read_f32::<T>()?,
        ))
    }
}

impl<R: io::Read + ?Sized> ReaderExt for R {}

impl StaticMesh {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::core::mesh::Result<Self> {
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
            return Err(ParseError::InvalidFileVersion);
        }

        let name = reader.read_padded_string::<LittleEndian, 128>()?;
        debug!("name: {name}");

        let vertex_count = reader.read_i32::<LittleEndian>()?;
        let face_count = reader.read_i32::<LittleEndian>()?;

        let flags = reader.read_u32::<LittleEndian>()?; // TODO (alan): handle StaticMeshFlags
        let bounding_box = reader.read_bbox_f32::<LittleEndian>()?;

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

        let central_point = reader.read_vector3_f32::<LittleEndian>()?;

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
