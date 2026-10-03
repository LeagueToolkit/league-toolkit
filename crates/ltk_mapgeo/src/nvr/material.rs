//! `.nvr` materials

use std::io::Read;

use bitflags::bitflags;
use byteorder::{ReadBytesExt, LE};
use glam::Mat4;
use ltk_io_ext::ReaderExt;
use ltk_mesh::mem::{VertexBufferDescription, VertexBufferUsage, VertexElement};
use ltk_primitives::Color;

use super::NAME_LENGTH;
use crate::NvrError;

/// What a material draws. The type also fixes the vertex layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MaterialType {
    /// Lit opaque geometry.
    Default,
    /// Geometry laid over the ground.
    Decal,
    /// Brush geometry.
    WallOfGrass,
    /// Terrain blending four textures through a second UV channel.
    FourBlend,
    /// Geometry that hides brush.
    AntiBrush,
}

impl MaterialType {
    fn from_raw(raw: i32, material: usize) -> Result<Self, NvrError> {
        match raw {
            0 => Ok(Self::Default),
            1 => Ok(Self::Decal),
            2 => Ok(Self::WallOfGrass),
            3 => Ok(Self::FourBlend),
            4 => Ok(Self::AntiBrush),
            kind => Err(NvrError::UnknownMaterialType { material, kind }),
        }
    }
}

bitflags! {
    /// Material flags. Version 8.1 files store none.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
    pub struct MaterialFlags: u32 {
        /// The material is terrain.
        const GROUND = 1 << 0;
        /// The geometry casts no shadow.
        const NO_SHADOW = 1 << 1;
        /// The vertex color alpha cuts the surface out.
        const VERTEX_ALPHA = 1 << 2;
        /// The geometry is lightmapped.
        const LIGHTMAPPED = 1 << 3;
        /// Vertices carry a secondary color.
        const DUAL_VERTEX_COLOR = 1 << 4;
        /// The geometry is background scenery.
        const BACKGROUND = 1 << 5;
        /// The geometry is background scenery that fog covers.
        const BACKGROUND_WITH_FOG = 1 << 6;

        const _ = !0;
    }
}

/// A texture slot of a material.
#[derive(Debug, Clone, PartialEq)]
pub struct Channel {
    color: Color<f32>,
    texture: String,
    transform: Mat4,
}

impl Channel {
    fn from_reader<R: Read + ?Sized>(reader: &mut R) -> Result<Self, NvrError> {
        let color = reader.read_color_f32::<LE>()?;
        let texture = reader.read_padded_string::<LE, NAME_LENGTH>()?;
        let transform = reader.read_mat4_row_major::<LE>()?;
        Ok(Self {
            color,
            texture,
            transform,
        })
    }

    fn from_reader_v8<R: Read + ?Sized>(reader: &mut R) -> Result<Self, NvrError> {
        let color = reader.read_color_f32::<LE>()?;
        let texture = reader.read_padded_string::<LE, NAME_LENGTH>()?;
        Ok(Self {
            color,
            texture,
            transform: Mat4::IDENTITY,
        })
    }

    /// The color the texture is multiplied by.
    #[inline]
    pub fn color(&self) -> Color<f32> {
        self.color
    }

    /// The texture file name as stored, usually a `.tga` name. An unused slot stores an empty
    /// name.
    #[inline]
    pub fn texture(&self) -> &str {
        &self.texture
    }

    /// The UV transform. A version 8.1 file stores none and reads as identity.
    #[inline]
    pub fn transform(&self) -> &Mat4 {
        &self.transform
    }
}

/// An `.nvr` material.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    name: String,
    kind: MaterialType,
    flags: MaterialFlags,
    channels: Vec<Channel>,
}

impl Material {
    /// Channels a version 9.1 material stores.
    pub const CHANNEL_COUNT: usize = 8;

    /// Index of the diffuse channel.
    pub const DIFFUSE: usize = 0;

    /// Index of the emissive channel.
    pub const EMISSIVE: usize = 1;

    pub(super) fn from_reader<R: Read + ?Sized>(
        reader: &mut R,
        index: usize,
    ) -> Result<Self, NvrError> {
        let name = reader.read_padded_string::<LE, NAME_LENGTH>()?;
        let kind = MaterialType::from_raw(reader.read_i32::<LE>()?, index)?;
        let flags = MaterialFlags::from_bits_retain(reader.read_u32::<LE>()?);
        let channels = (0..Self::CHANNEL_COUNT)
            .map(|_| Channel::from_reader(reader))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            name,
            kind,
            flags,
            channels,
        })
    }

    /// Reads the version 8.1 layout: no flags, and only the diffuse and emissive channels.
    pub(super) fn from_reader_v8<R: Read + ?Sized>(
        reader: &mut R,
        index: usize,
    ) -> Result<Self, NvrError> {
        let name = reader.read_padded_string::<LE, NAME_LENGTH>()?;
        let kind = MaterialType::from_raw(reader.read_i32::<LE>()?, index)?;
        let diffuse = Channel::from_reader_v8(reader)?;
        let emissive = Channel::from_reader_v8(reader)?;
        Ok(Self {
            name,
            kind,
            flags: MaterialFlags::empty(),
            channels: vec![diffuse, emissive],
        })
    }

    /// The material name.
    #[inline]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The material type.
    #[inline]
    pub fn kind(&self) -> MaterialType {
        self.kind
    }

    /// The material flags.
    #[inline]
    pub fn flags(&self) -> MaterialFlags {
        self.flags
    }

    /// The texture slots: [`CHANNEL_COUNT`](Self::CHANNEL_COUNT) in version 9.1, two in 8.1.
    #[inline]
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// The diffuse texture file name, or empty.
    pub fn diffuse_texture(&self) -> &str {
        self.channels.get(Self::DIFFUSE).map_or("", |c| c.texture())
    }

    /// The vertex layout of the full-detail geometry drawn with this material.
    pub fn vertex_description(&self) -> VertexBufferDescription {
        let mut elements = vec![
            VertexElement::POSITION,
            VertexElement::NORMAL,
            VertexElement::TEXCOORD_0,
        ];
        if self.kind == MaterialType::FourBlend {
            elements.push(VertexElement::TEXCOORD_7);
        }
        elements.push(VertexElement::PRIMARY_COLOR);
        if self.kind == MaterialType::Default
            && self.flags.contains(MaterialFlags::DUAL_VERTEX_COLOR)
        {
            elements.push(VertexElement::SECONDARY_COLOR);
        }
        VertexBufferDescription::new(VertexBufferUsage::Static, elements)
    }
}
