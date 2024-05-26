use std::io::{self, Read};

use super::ReaderExt;
use byteorder::{ByteOrder, LittleEndian, ReadBytesExt};

use vecmath::Vector2;

use crate::core::primitives::Color;

#[derive(Debug, Clone)]
pub struct StaticMeshFace {
    pub material: String,
    pub vertex_ids: (u8, u8, u8),
    pub uvs: (Vector2<f32>, Vector2<f32>, Vector2<f32>),
    pub colors: (Color, Color, Color),
}

impl StaticMeshFace {
    pub fn from_reader<R: Read>(reader: &mut R) -> super::Result<Self> {
        let vertex_ids = (
            reader.read_u32::<LittleEndian>()? as u8,
            reader.read_u32::<LittleEndian>()? as u8,
            reader.read_u32::<LittleEndian>()? as u8,
        );

        let material = reader.read_padded_string::<LittleEndian, 64>()?;

        let uvs = (
            reader.read_f32::<LittleEndian>()?,
            reader.read_f32::<LittleEndian>()?,
            reader.read_f32::<LittleEndian>()?,
            reader.read_f32::<LittleEndian>()?,
            reader.read_f32::<LittleEndian>()?,
            reader.read_f32::<LittleEndian>()?,
        );

        Ok(Self {
            material,
            vertex_ids,
            uvs: ([uvs.0, uvs.3], [uvs.1, uvs.4], [uvs.2, uvs.5]),
            colors: (Color::ONE, Color::ONE, Color::ONE),
        })
    }
}
