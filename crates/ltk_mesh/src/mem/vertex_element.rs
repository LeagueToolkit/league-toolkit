use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Which attribute a vertex element carries.
///
/// Mirrors `Riot::Renderer::Mesh::Elem`. The trailing comment on each variant is the X3D
/// stream index the game maps it to.
#[repr(u32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive, IntoPrimitive,
)]
pub enum ElementName {
    /// Vertex position. StreamIndex 0.
    Position,
    /// Skinning weights. StreamIndex 1.
    BlendWeight,
    /// Vertex normal. StreamIndex 2.
    Normal,
    /// Unused by the game (no stream mapping).
    FogCoordinate,
    /// Primary vertex colour. StreamIndex 3.
    PrimaryColor,
    /// Secondary vertex colour. StreamIndex 4.
    SecondaryColor,
    /// Skinning indices. StreamIndex 7.
    BlendIndex,
    /// Diffuse UV channel. StreamIndex 8.
    Texcoord0,
    /// UV channel 1. StreamIndex 9.
    Texcoord1,
    /// UV channel 2. StreamIndex 10.
    Texcoord2,
    /// UV channel 3. StreamIndex 11.
    Texcoord3,
    /// UV channel 4. StreamIndex 12.
    Texcoord4,
    /// UV channel 5. StreamIndex 13.
    Texcoord5,
    /// UV channel 6. StreamIndex 14, which also carries tangents.
    Texcoord6,
    /// UV channel 7, the lightmap UV. StreamIndex 15.
    Texcoord7,
}

/// How a vertex element is packed.
///
/// Mirrors `Riot::Renderer::Mesh::ElemFormat`. The game rejects any value above 8, treating
/// it as a zero-size "none" element.
#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive, IntoPrimitive,
)]
pub enum ElementFormat {
    /// One `f32`.
    X_Float32,
    /// Two `f32`.
    XY_Float32,
    /// Three `f32`.
    XYZ_Float32,
    /// Four `f32`.
    XYZW_Float32,
    /// Four `u8`, blue first.
    BGRA_Packed8888,
    /// Four `u8`, red first. Same GPU format as BGRA; the swizzle is handled in shaders.
    RGBA_Packed8888,
    /// Four `u8`, used for blend indices.
    UByte4,
    /// Two `f16`.
    XY_Float16,
    /// Four `f16`.
    XYZW_Float16,
}

impl ElementFormat {
    /// The size in bytes of one element in this format.
    #[must_use]
    pub fn size(&self) -> usize {
        match self {
            ElementFormat::X_Float32 => 4,
            ElementFormat::XY_Float32 => 8,
            ElementFormat::XYZ_Float32 => 12,
            ElementFormat::XYZW_Float32 => 16,
            ElementFormat::BGRA_Packed8888 => 4,
            ElementFormat::RGBA_Packed8888 => 4,
            ElementFormat::UByte4 => 4,
            ElementFormat::XY_Float16 => 4,
            ElementFormat::XYZW_Float16 => 8,
        }
    }
}

/// One attribute of a vertex: what it is, and how it is packed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexElement {
    /// Which attribute this element carries.
    pub name: ElementName,
    /// How the attribute is packed.
    pub format: ElementFormat,
}

impl VertexElement {
    /// `Vec3` position.
    pub const POSITION: Self = Self::new(ElementName::Position, ElementFormat::XYZ_Float32);
    /// `Vec4` of skinning weights.
    pub const BLEND_WEIGHT: Self = Self::new(ElementName::BlendWeight, ElementFormat::XYZW_Float32);
    /// `Vec3` normal.
    pub const NORMAL: Self = Self::new(ElementName::Normal, ElementFormat::XYZ_Float32);
    /// Single `f32` fog coordinate.
    pub const FOG_COORDINATE: Self =
        Self::new(ElementName::FogCoordinate, ElementFormat::X_Float32);
    /// Packed BGRA primary colour.
    pub const PRIMARY_COLOR: Self =
        Self::new(ElementName::PrimaryColor, ElementFormat::BGRA_Packed8888);
    /// Packed BGRA secondary colour.
    pub const SECONDARY_COLOR: Self =
        Self::new(ElementName::SecondaryColor, ElementFormat::BGRA_Packed8888);
    /// Four `u8` skinning indices.
    pub const BLEND_INDEX: Self = Self::new(ElementName::BlendIndex, ElementFormat::UByte4);
    /// `Vec2` diffuse UV.
    pub const TEXCOORD_0: Self = Self::new(ElementName::Texcoord0, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 1.
    pub const TEXCOORD_1: Self = Self::new(ElementName::Texcoord1, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 2.
    pub const TEXCOORD_2: Self = Self::new(ElementName::Texcoord2, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 3.
    pub const TEXCOORD_3: Self = Self::new(ElementName::Texcoord3, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 4.
    pub const TEXCOORD_4: Self = Self::new(ElementName::Texcoord4, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 5.
    pub const TEXCOORD_5: Self = Self::new(ElementName::Texcoord5, ElementFormat::XY_Float32);
    /// `Vec2` UV channel 6.
    pub const TEXCOORD_6: Self = Self::new(ElementName::Texcoord6, ElementFormat::XY_Float32);
    /// `Vec2` lightmap UV.
    pub const TEXCOORD_7: Self = Self::new(ElementName::Texcoord7, ElementFormat::XY_Float32);
    /// `Vec4` tangent, which shares stream 14 with [`VertexElement::TEXCOORD_6`].
    pub const TANGENT: Self = Self::new(ElementName::Texcoord6, ElementFormat::XYZW_Float32);

    /// Pairs an attribute with the format it is packed in.
    #[must_use]
    pub const fn new(name: ElementName, format: ElementFormat) -> Self {
        Self { name, format }
    }

    /// The size in bytes of this element.
    #[must_use]
    pub fn size(&self) -> usize {
        self.format.size()
    }
}
