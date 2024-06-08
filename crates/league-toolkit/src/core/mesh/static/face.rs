use std::io::Read;

use byteorder::{LittleEndian, ReadBytesExt};
use glam::{Vec2, vec2};

use crate::core::primitives::Color;

#[derive(Debug, Clone)]
pub struct StaticMeshFace {
    pub material: String,
    pub vertex_ids: (u8, u8, u8),
    pub uvs: (Vec2, Vec2, Vec2),
    pub colors: (Color, Color, Color),
}

impl StaticMeshFace {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::core::mesh::Result<Self> {
        use crate::util::ReaderExt as _;
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
            uvs: (vec2(uvs.0, uvs.3), vec2(uvs.1, uvs.4), vec2(uvs.2, uvs.5)),
            colors: (Color::ONE, Color::ONE, Color::ONE),
        })
    }
}
