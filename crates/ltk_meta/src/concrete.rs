//! Concrete aliases for the value model, with no per-value metadata.
//!
//! Every type in the value model is generic over a metadata payload `M`, defaulted to
//! [`NoMeta`]. Rust applies that default in type position but never in expression
//! position, so spelling the generic names in a `let` demands an annotation or a
//! turbofish. A path through these aliases pins `M = NoMeta` before inference starts,
//! which makes the parameter disappear entirely:
//!
//! ```
//! use ltk_meta::concrete::{values, Bin, BinObject};
//!
//! // Uninferrable through the generic names; fine through the aliases.
//! let list = values::Container::from(vec![values::I32::new(1), values::I32::new(2)]);
//!
//! let bin = Bin::builder()
//!     .object(
//!         BinObject::builder(0x1111u32, 0x2222u32)
//!             .property(0x3333u32, values::F32::new(1.5))
//!             .property(0x4444u32, list)
//!             .build(),
//!     )
//!     .build();
//! # assert_eq!(bin.objects.len(), 1);
//! ```
//!
//! Code that carries metadata (such as `ltk_ritobin`, which threads source spans)
//! keeps using the generic types directly.
//!
//! [`NoMeta`]: crate::property::NoMeta

pub type Bin = crate::Bin;
pub type BinObject = crate::BinObject;
pub type BinStream<R> = crate::stream::BinStream<R>;
pub type BinFile = crate::BinFile;
pub type BinOverride = crate::BinOverride;
pub type PropertyPatch = crate::PropertyPatch;
pub type PropertyValueEnum = crate::PropertyValueEnum;
pub type ValueSlot<'a> = crate::ValueSlot<'a>;

/// Non-generic aliases for [`crate::property::values`].
pub mod values {
    use crate::property::values as v;

    pub type Bool = v::Bool;
    pub type BitBool = v::BitBool;
    pub type I8 = v::I8;
    pub type U8 = v::U8;
    pub type I16 = v::I16;
    pub type U16 = v::U16;
    pub type I32 = v::I32;
    pub type U32 = v::U32;
    pub type I64 = v::I64;
    pub type U64 = v::U64;
    pub type F32 = v::F32;
    pub type Vector2 = v::Vector2;
    pub type Vector3 = v::Vector3;
    pub type Vector4 = v::Vector4;
    pub type Matrix44 = v::Matrix44;
    pub type Color = v::Color;
    pub type Hash = v::Hash;
    pub type WadChunkLink = v::WadChunkLink;
    pub type ObjectLink = v::ObjectLink;
    pub type String = v::String;
    pub type Struct = v::Struct;
    pub type Embedded = v::Embedded;
    pub type None = v::None;
    pub type Container = v::Container;
    pub type UnorderedContainer = v::UnorderedContainer;
    pub type Optional = v::Optional;
    pub type Map = v::Map;
}
