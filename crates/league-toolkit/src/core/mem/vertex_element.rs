use num_enum::{IntoPrimitive, TryFromPrimitive};

// Riot::Renderer::Mesh::Elem
#[repr(u32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive, IntoPrimitive,
)]
pub enum ElementName {
    Position,       // StreamIndex -> 0
    BlendWeight,    // StreamIndex -> 1
    Normal,         // StreamIndex -> 2
    FogCoordinate,  // StreamIndex -> 5
    PrimaryColor,   // StreamIndex -> 3
    SecondaryColor, // StreamIndex -> 4
    BlendIndex,     // StreamIndex -> 7
    Texcoord0,      // StreamIndex -> 8
    Texcoord1,      // StreamIndex -> 9
    Texcoord2,      // StreamIndex -> 10
    Texcoord3,      // StreamIndex -> 11
    Texcoord4,      // StreamIndex -> 12
    Texcoord5,      // StreamIndex -> 13
    Texcoord6,      // StreamIndex -> 14
    Texcoord7,      // StreamIndex -> 15

    Tangent, // Riot's enum doesn't have this so not sure how they map it
}

//Riot::Renderer::Mesh::ElemFormat
// Riot::Renderer::Mesh::ElemFormat
#[allow(non_camel_case_types)]
#[repr(u32)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive, IntoPrimitive,
)]
pub enum ElementFormat {
    X_Float32,
    XY_Float32,
    XYZ_Float32,
    XYZW_Float32,
    BGRA_Packed8888,
    ZYXW_Packed8888,
    RGBA_Packed8888,
    XYZW_Packed8888,
}

impl ElementFormat {
    pub fn size(&self) -> usize {
        match self {
            ElementFormat::X_Float32 => 4,
            ElementFormat::XY_Float32 => 8,
            ElementFormat::XYZ_Float32 => 12,
            ElementFormat::XYZW_Float32 => 16,
            ElementFormat::BGRA_Packed8888 => 4,
            ElementFormat::ZYXW_Packed8888 => 4,
            ElementFormat::RGBA_Packed8888 => 4,
            ElementFormat::XYZW_Packed8888 => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VertexElement {
    pub name: ElementName,
    pub format: ElementFormat,
}

impl VertexElement {
    pub const POSITION: Self = Self::new(ElementName::Position, ElementFormat::XYZ_Float32);
    pub const BLEND_WEIGHT: Self = Self::new(ElementName::BlendWeight, ElementFormat::XYZW_Float32);
    pub const NORMAL: Self = Self::new(ElementName::Normal, ElementFormat::XYZ_Float32);
    pub const FOG_COORDINATE: Self =
        Self::new(ElementName::FogCoordinate, ElementFormat::X_Float32);
    pub const PRIMARY_COLOR: Self =
        Self::new(ElementName::PrimaryColor, ElementFormat::BGRA_Packed8888);
    pub const SECONDARY_COLOR: Self =
        Self::new(ElementName::SecondaryColor, ElementFormat::BGRA_Packed8888);
    pub const BLEND_INDEX: Self =
        Self::new(ElementName::BlendIndex, ElementFormat::XYZW_Packed8888);
    pub const TEXCOORD_0: Self = Self::new(ElementName::Texcoord0, ElementFormat::XY_Float32);
    pub const TEXCOORD_1: Self = Self::new(ElementName::Texcoord1, ElementFormat::XY_Float32);
    pub const TEXCOORD_2: Self = Self::new(ElementName::Texcoord2, ElementFormat::XY_Float32);
    pub const TEXCOORD_3: Self = Self::new(ElementName::Texcoord3, ElementFormat::XY_Float32);
    pub const TEXCOORD_4: Self = Self::new(ElementName::Texcoord4, ElementFormat::XY_Float32);
    pub const TEXCOORD_5: Self = Self::new(ElementName::Texcoord5, ElementFormat::XY_Float32);
    pub const TEXCOORD_6: Self = Self::new(ElementName::Texcoord6, ElementFormat::XY_Float32);
    pub const TEXCOORD_7: Self = Self::new(ElementName::Texcoord7, ElementFormat::XY_Float32);
    pub const TANGENT: Self = Self::new(ElementName::Tangent, ElementFormat::XYZW_Float32);
    pub const fn new(name: ElementName, format: ElementFormat) -> Self {
        Self { name, format }
    }

    pub fn size(&self) -> usize {
        self.format.size()
    }
}
