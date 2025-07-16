use byteorder::{ByteOrder, ReadBytesExt, WriteBytesExt};
use std::io::{self, Write};

/// Generic RGBA Color struct
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Color<T = f32> {
    pub r: T,
    pub g: T,
    pub b: T,
    pub a: T,
}

impl<T> AsRef<Color<T>> for Color<T> {
    fn as_ref(&self) -> &Color<T> {
        self
    }
}

impl<T> Color<T> {
    pub const fn new(r: T, g: T, b: T, a: T) -> Self {
        Self { r, g, b, a }
    }
}

impl Color<u8> {
    pub const ONE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    pub fn to_writer(&self, writer: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        writer.write_u8(self.r)?;
        writer.write_u8(self.g)?;
        writer.write_u8(self.b)?;
        writer.write_u8(self.a)?;
        Ok(())
    }

    #[inline]
    pub fn from_reader<R: io::Read + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok(Self {
            r: reader.read_u8()?,
            g: reader.read_u8()?,
            b: reader.read_u8()?,
            a: reader.read_u8()?,
        })
    }
}

impl Color<f32> {
    pub const ONE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
    pub fn to_writer<E: ByteOrder>(
        &self,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        writer.write_f32::<E>(self.r)?;
        writer.write_f32::<E>(self.g)?;
        writer.write_f32::<E>(self.b)?;
        writer.write_f32::<E>(self.a)?;
        Ok(())
    }

    #[inline]
    pub fn from_reader<E: ByteOrder, R: io::Read + ?Sized>(reader: &mut R) -> io::Result<Self> {
        Ok(Self {
            r: reader.read_f32::<E>()?,
            g: reader.read_f32::<E>()?,
            b: reader.read_f32::<E>()?,
            a: reader.read_f32::<E>()?,
        })
    }
}

// TODO (alan): finish Color impl
