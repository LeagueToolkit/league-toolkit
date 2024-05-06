use std::io::{self, Read, Write};

use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt};
use vecmath::{Vector2, Vector3, Vector4};

use crate::core::primitives::{Color, Sphere, AABB};
pub trait WriterExt: Write {
    fn write_padded_string<const N: usize>(&mut self, str: &str) -> io::Result<()> {
        debug_assert!(str.len() <= N);
        let mut buf = Vec::with_capacity(N);
        buf.extend_from_slice(str.as_bytes());
        buf.resize(N, 0);
        self.write_all(&buf)
    }

    fn write_color<E: ByteOrder>(&mut self, color: &Color) -> io::Result<()> {
        color.to_writer::<E>(self)
    }

    fn write_vector2_f32<E: ByteOrder>(&mut self, vec: &Vector2<f32>) -> io::Result<()> {
        self.write_f32::<E>(vec[0])?;
        self.write_f32::<E>(vec[1])?;
        Ok(())
    }
    fn write_vector3_f32<E: ByteOrder>(&mut self, vec: &Vector3<f32>) -> io::Result<()> {
        self.write_f32::<E>(vec[0])?;
        self.write_f32::<E>(vec[1])?;
        self.write_f32::<E>(vec[2])?;
        Ok(())
    }
    fn write_vector4_f32<E: ByteOrder>(&mut self, vec: &Vector4<f32>) -> io::Result<()> {
        self.write_f32::<E>(vec[0])?;
        self.write_f32::<E>(vec[1])?;
        self.write_f32::<E>(vec[2])?;
        self.write_f32::<E>(vec[3])?;
        Ok(())
    }
    fn write_quaternion_f32<E: ByteOrder>(&mut self, quaternion: &Vector4<f32>) -> io::Result<()> {
        self.write_f32::<E>(quaternion[0])?;
        self.write_f32::<E>(quaternion[1])?;
        self.write_f32::<E>(quaternion[2])?;
        self.write_f32::<E>(quaternion[3])?;
        Ok(())
    }


    fn write_aabb_f32<E: ByteOrder>(&mut self, aabb: &AABB<f32>) -> io::Result<()> {
        self.write_vector3_f32::<E>(&aabb.min)?;
        self.write_vector3_f32::<E>(&aabb.max)?;
        Ok(())
    }
    fn write_sphere_f32<E: ByteOrder>(&mut self, sphere: &Sphere) -> io::Result<()> {
        self.write_vector3_f32::<E>(&sphere.origin)?;
        self.write_f32::<E>(sphere.radius)?;
        Ok(())
    }
}

impl<W: Write + ?Sized> WriterExt for W {}
