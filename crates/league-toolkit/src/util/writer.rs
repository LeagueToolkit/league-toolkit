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

    /// Writes a string with a null terminator (writes sizeof(str) + 1 bytes)
    fn write_terminated_string<S: AsRef<str>>(&mut self, str: S) -> io::Result<()> {
        self.write_all(str.as_ref().as_bytes())?;
        self.write_u8(0)
    }

    fn write_color<E: ByteOrder>(&mut self, color: &Color) -> io::Result<()> {
        color.to_writer::<E>(self)
    }

    fn write_vec2<E: ByteOrder>(&mut self, vec: &Vec2) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_vec3<E: ByteOrder>(&mut self, vec: &Vec3) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_vec4<E: ByteOrder>(&mut self, vec: &Vec4) -> io::Result<()> {
        for i in vec.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }
    fn write_quat<E: ByteOrder>(&mut self, quaternion: &Quat) -> io::Result<()> {
        for i in quaternion.to_array() {
            self.write_f32::<E>(i)?;
        }
        Ok(())
    }

    fn write_aabb<E: ByteOrder>(&mut self, aabb: &AABB) -> io::Result<()> {
        self.write_vec3::<E>(&aabb.min)?;
        self.write_vec3::<E>(&aabb.max)?;
        Ok(())
    }
    fn write_sphere<E: ByteOrder>(&mut self, sphere: &Sphere) -> io::Result<()> {
        self.write_vec3::<E>(&sphere.origin)?;
        self.write_f32::<E>(sphere.radius)?;
        Ok(())
    }
}

impl<W: Write + ?Sized> WriterExt for W {}
