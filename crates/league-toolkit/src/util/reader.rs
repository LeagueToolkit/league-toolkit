use std::io::{self, Read};

use byteorder::{ByteOrder, ReadBytesExt};
use vecmath::{Vector2, Vector3, Vector4};

use crate::core::primitives::{Color, Sphere, AABB};
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

impl<R: Read + ?Sized> ReaderExt for R {}
