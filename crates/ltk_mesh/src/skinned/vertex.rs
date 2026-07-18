use lazy_static::lazy_static;

use crate::mem::{VertexBufferDescription, VertexBufferUsage, VertexElement};
lazy_static! {
    pub static ref BASIC: VertexBufferDescription = VertexBufferDescription::new(
        VertexBufferUsage::Static,
        vec![
            VertexElement::POSITION,
            VertexElement::BLEND_INDEX,
            VertexElement::BLEND_WEIGHT,
            VertexElement::NORMAL,
            VertexElement::TEXCOORD_0,
        ],
    );
    pub static ref COLOR: VertexBufferDescription = VertexBufferDescription::new(
        VertexBufferUsage::Static,
        vec![
            VertexElement::POSITION,
            VertexElement::BLEND_INDEX,
            VertexElement::BLEND_WEIGHT,
            VertexElement::NORMAL,
            VertexElement::TEXCOORD_0,
            VertexElement::PRIMARY_COLOR,
        ],
    );
    pub static ref TANGENT: VertexBufferDescription = VertexBufferDescription::new(
        VertexBufferUsage::Static,
        vec![
            VertexElement::POSITION,
            VertexElement::BLEND_INDEX,
            VertexElement::BLEND_WEIGHT,
            VertexElement::NORMAL,
            VertexElement::TEXCOORD_0,
            VertexElement::PRIMARY_COLOR,
            VertexElement::TANGENT,
        ],
    );
    /// Extended vertex type 3 (104 bytes): 4 extra float2 UV channels (Texcoord1-4)
    /// inserted after Texcoord0; color and tangent follow at +84/+88.
    pub static ref EXT: VertexBufferDescription = VertexBufferDescription::new(
        VertexBufferUsage::Static,
        vec![
            VertexElement::POSITION,
            VertexElement::BLEND_INDEX,
            VertexElement::BLEND_WEIGHT,
            VertexElement::NORMAL,
            VertexElement::TEXCOORD_0,
            VertexElement::TEXCOORD_1,
            VertexElement::TEXCOORD_2,
            VertexElement::TEXCOORD_3,
            VertexElement::TEXCOORD_4,
            VertexElement::PRIMARY_COLOR,
            VertexElement::TANGENT,
        ],
    );
}
