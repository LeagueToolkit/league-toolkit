use byteorder::{ByteOrder, WriteBytesExt};
use std::io::Write;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const ONE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

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
}

// TODO (alan): finish Color impl
