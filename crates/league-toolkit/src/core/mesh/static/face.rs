use io_ext::ReaderExt;
use std::io::Read;

use byteorder::{ReadBytesExt, LE};
use glam::{vec2, Vec2};

use league_primitives::Color;

#[derive(Debug, Clone)]
pub struct StaticMeshFace {
    pub material: String,
    pub vertex_ids: (u8, u8, u8),
    pub uvs: (Vec2, Vec2, Vec2),
    pub colors: (Color, Color, Color),
}

impl StaticMeshFace {
    pub fn from_reader<R: Read>(reader: &mut R) -> crate::core::mesh::Result<Self> {
        let vertex_ids = (
            reader.read_u32::<LE>()? as u8,
            reader.read_u32::<LE>()? as u8,
            reader.read_u32::<LE>()? as u8,
        );

        let material = reader.read_padded_string::<LE, 64>()?;

        let uvs = (
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
            reader.read_f32::<LE>()?,
        );

        Ok(Self {
            material,
            vertex_ids,
            uvs: (vec2(uvs.0, uvs.3), vec2(uvs.1, uvs.4), vec2(uvs.2, uvs.5)),
            colors: (Color::<f32>::ONE, Color::<f32>::ONE, Color::<f32>::ONE),
        })
    }
}
