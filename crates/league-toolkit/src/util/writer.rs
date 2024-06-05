use std::io::{self, Read, Write};

use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt};
use glam::{Quat, Vec2, Vec3, Vec4};

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

    fn write_vector2_f32<E: ByteOrder>(&mut self, vec: &Vec2) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_vector3_f32<E: ByteOrder>(&mut self, vec: &Vec3) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_vector4_f32<E: ByteOrder>(&mut self, vec: &Vec4) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_quaternion_f32<E: ByteOrder>(&mut self, quaternion: &Quat) -> io::Result<()> {
        for i in quaternion.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }


    fn write_aabb_f32<E: ByteOrder>(&mut self, aabb: &AABB) -> io::Result<()> {
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
