//! Type name mappings for ritobin format.

use ltk_hash::{HashAlgorithm, HashValue, HashWidth, Hasher};
use ltk_meta::{property::values, PropertyKind, PropertyValueEnum};
use std::fmt::Display;

mod hash;

mod container;
mod rito_type;
mod scalar;

pub use container::*;
pub use hash::*;
pub use rito_type::*;
pub use scalar::*;

/// Extension trait for mapping ritobin type names to/from [`PropertyKind`]'s
pub trait RitobinName {
    /// Maps a ritobin type name string to a [`ltk_meta::PropertyKind`].
    /// **NOTE:** Case sensitive.
    fn from_rito_name(name: &str) -> Option<Self>
    where
        Self: Sized;

    /// Maps a [`ltk_meta::PropertyKind`] to its ritobin type name string.
    fn to_rito_name(&self) -> &'static str;
}

impl RitobinName for PropertyKind {
    fn from_rito_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(Self::None),
            "bool" => Some(Self::Bool),
            "i8" => Some(Self::I8),
            "u8" => Some(Self::U8),
            "i16" => Some(Self::I16),
            "u16" => Some(Self::U16),
            "i32" => Some(Self::I32),
            "u32" => Some(Self::U32),
            "i64" => Some(Self::I64),
            "u64" => Some(Self::U64),
            "f32" => Some(Self::F32),
            "vec2" => Some(Self::Vector2),
            "vec3" => Some(Self::Vector3),
            "vec4" => Some(Self::Vector4),
            "mtx44" => Some(Self::Matrix44),
            "rgba" => Some(Self::Color),
            "string" => Some(Self::String),
            "hash" => Some(Self::Hash),
            "file" => Some(Self::WadChunkLink),
            "list" => Some(Self::Container),
            "list2" => Some(Self::UnorderedContainer),
            "pointer" => Some(Self::Struct),
            "embed" => Some(Self::Embedded),
            "link" => Some(Self::ObjectLink),
            "option" => Some(Self::Optional),
            "map" => Some(Self::Map),
            "flag" => Some(Self::BitBool),
            _ => None,
        }
    }

    fn to_rito_name(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Bool => "bool",
            Self::I8 => "i8",
            Self::U8 => "u8",
            Self::I16 => "i16",
            Self::U16 => "u16",
            Self::I32 => "i32",
            Self::U32 => "u32",
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::F32 => "f32",
            Self::Vector2 => "vec2",
            Self::Vector3 => "vec3",
            Self::Vector4 => "vec4",
            Self::Matrix44 => "mtx44",
            Self::Color => "rgba",
            Self::String => "string",
            Self::Hash => "hash",
            Self::WadChunkLink => "file",
            Self::Container => "list",
            Self::UnorderedContainer => "list2",
            Self::Struct => "pointer",
            Self::Embedded => "embed",
            Self::ObjectLink => "link",
            Self::Optional => "option",
            Self::Map => "map",
            Self::BitBool => "flag",
        }
    }
}

#[macro_export]
macro_rules! rito {
    ($kind:ident) => {
        const {
            match $crate::RitoType::try_from_scalar(ltk_meta::PropertyKind::$kind) {
                Ok(t) => t,
                Err(_) => panic!("invalid simple rito type"),
            }
        }
    };
    ($kind:ident [ $sub:ident ]) => {
        const {
            match $crate::RitoType::try_from_parts(
                ltk_meta::PropertyKind::$kind,
                [Some(ltk_meta::PropertyKind::$sub), None],
            ) {
                Ok(t) => t,
                Err(_) => panic!("invalid rito type"),
            }
        }
    };
    ($kind:ident [ $a:ident, $b:ident ]) => {
        const {
            match $crate::RitoType::try_from_parts(
                ltk_meta::PropertyKind::$kind,
                [
                    Some(ltk_meta::PropertyKind::$a),
                    Some(ltk_meta::PropertyKind::$b),
                ],
            ) {
                Ok(t) => t,
                Err(_) => panic!("invalid rito type"),
            }
        }
    };
}

impl RitoType {
    /// Creates a RitoType, from the given [`PropertyKind`]. Container types will have their
    /// subtypes as [`Option::None`], and hashes will use [`HashType::default`]
    #[inline(always)]
    #[must_use]
    pub const fn from_property_kind(kind: PropertyKind) -> Self {
        use ContainerType as C;
        use ScalarType as S;
        match kind {
            PropertyKind::None => Self::Scalar(S::None),
            PropertyKind::Bool => Self::Scalar(S::Bool),
            PropertyKind::I8 => Self::Scalar(S::I8),
            PropertyKind::U8 => Self::Scalar(S::U8),
            PropertyKind::I16 => Self::Scalar(S::I16),
            PropertyKind::U16 => Self::Scalar(S::U16),
            PropertyKind::I32 => Self::Scalar(S::I32),
            PropertyKind::U32 => Self::Scalar(S::U32),
            PropertyKind::I64 => Self::Scalar(S::I64),
            PropertyKind::U64 => Self::Scalar(S::U64),
            PropertyKind::F32 => Self::Scalar(S::F32),
            PropertyKind::Vector2 => Self::Scalar(S::Vector2),
            PropertyKind::Vector3 => Self::Scalar(S::Vector3),
            PropertyKind::Vector4 => Self::Scalar(S::Vector4),
            PropertyKind::Matrix44 => Self::Scalar(S::Matrix44),
            PropertyKind::Color => Self::Scalar(S::Color),
            PropertyKind::String => Self::Scalar(S::String),
            PropertyKind::WadChunkLink => Self::Scalar(S::WadChunkLink),
            PropertyKind::Struct => Self::Scalar(S::Struct),
            PropertyKind::Embedded => Self::Scalar(S::Embedded),
            PropertyKind::ObjectLink => Self::Scalar(S::ObjectLink),
            PropertyKind::BitBool => Self::Scalar(S::BitBool),

            PropertyKind::Hash => Self::Scalar(S::Hash(HashType::DEFAULT)),

            PropertyKind::Container => Self::Container(C::Container(None)),
            PropertyKind::UnorderedContainer => Self::Container(C::UnorderedContainer(None)),
            PropertyKind::Optional => Self::Container(C::Optional(None)),
            PropertyKind::Map => Self::Container(C::Map {
                key: None,
                value: None,
            }),
        }
    }
}

impl RitoType {
    #[inline(always)]
    #[must_use]
    pub const fn scalar(kind: ScalarType) -> Self {
        Self::Scalar(kind)
    }

    #[inline(always)]
    #[must_use]
    pub const fn container(kind: ContainerType) -> Self {
        Self::Container(kind)
    }

    pub const fn as_scalar(self) -> Option<ScalarType> {
        match self {
            RitoType::Scalar(kind) => Some(kind),
            _ => None,
        }
    }

    pub const fn as_container(self) -> Option<ContainerType> {
        match self {
            RitoType::Container(kind) => Some(kind),
            _ => None,
        }
    }

    pub fn equivalent_to(&self, other: &Self) -> bool {
        match (self, other) {
            (RitoType::Scalar(a), RitoType::Scalar(b)) => a == b,
            (RitoType::Container(a), RitoType::Container(b)) => a.equivalent_to(b),
            _ => false,
        }
    }

    /// Attempt to create a scalar [`RitoType`], from the given [`PropertyKind`], if that kind is
    /// not a container type.
    #[inline(always)]
    pub const fn try_from_scalar(kind: PropertyKind) -> Result<Self, BadScalarType> {
        match ScalarType::try_from_kind(kind) {
            Ok(v) => Ok(Self::Scalar(v)),
            Err(e) => Err(e),
        }
    }

    /// Returns the hash type of the base, if it is of type [`ScalarType::Hash`]
    pub const fn hash_type(&self) -> Option<HashType> {
        match self {
            RitoType::Scalar(ScalarType::Hash(hash_type)) => Some(*hash_type),
            _ => None,
        }
    }

    /// Returns the `n`th subtype, if present.
    pub const fn subtype(&self, n: usize) -> Option<ScalarType> {
        match self {
            RitoType::Scalar(_) => None,
            RitoType::Container(container) => container.subtype(n),
        }
    }

    /// Returns the type with [`HashType::DEFAULT`] in each slot.
    pub fn with_default_hash_types(self) -> Self {
        match self {
            RitoType::Scalar(scalar) => Self::Scalar(scalar.with_default_hash_type()),
            RitoType::Container(container) => Self::Container(container.with_default_hash_types()),
        }
    }
    /// Returns the type with the given hash type in any hash types/subtypes.
    pub fn with_hash_types(self, hash_types: [HashType; 2]) -> Self {
        match self {
            RitoType::Scalar(scalar) => Self::Scalar(scalar.with_hash_type(hash_types[0])),
            RitoType::Container(container) => {
                Self::Container(container.with_hash_types(hash_types))
            }
        }
    }

    pub const fn try_from_parts(
        base: PropertyKind,
        subtypes: [Option<PropertyKind>; 2],
    ) -> Result<Self, BadRitoType> {
        use BadRitoType as E;
        match subtypes[0] {
            Some(sub) => match ContainerType::try_from_parts(base, (sub, subtypes[1])) {
                Ok(container) => Ok(Self::Container(container)),
                Err(e) => Err(E::BadContainerType(e)),
            },
            None => match ScalarType::try_from_kind(base) {
                Ok(scalar) => Ok(Self::Scalar(scalar)),
                Err(e) => Err(E::BadScalarType(e)),
            },
        }
    }

    pub fn from_parts(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Self {
        ContainerType::from_parts(base, subtypes)
            .map(RitoType::from)
            .unwrap_or_else(|| {
                ScalarType::try_from_kind(base)
                    .map(RitoType::from)
                    .expect("ContainerType::from_parts prevents this")
            })
    }

    pub const fn ordered_list(value_type: ScalarType) -> Self {
        Self::Container(ContainerType::Container(Some(value_type)))
    }

    pub const fn map(key: ScalarType, value: ScalarType) -> Self {
        Self::Container(ContainerType::Map {
            key: Some(key),
            value: Some(value),
        })
    }

    /// The scalar type, or the base value if a container
    pub const fn base(&self) -> PropertyKind {
        match self {
            RitoType::Scalar(scalar_type) => scalar_type.kind(),
            RitoType::Container(container_type) => container_type.base_kind(),
        }
    }

    /// The value type of a map, item type of an {un,}ordered list/optional, or `None` if we're a scalar.
    pub const fn value_subtype(&self) -> Option<ScalarType> {
        match self {
            RitoType::Scalar(_) => return None,
            RitoType::Container(container) => match container {
                ContainerType::Map { value, .. } => *value,
                ContainerType::Container(value) => *value,
                ContainerType::UnorderedContainer(value) => *value,
                ContainerType::Optional(value) => *value,
            },
        }
    }

    /// Whether this type's `{ .. }` block holds entries or bare values.
    ///
    /// `None` for the types that are never written with a block at all.
    ///
    /// ```text
    /// Entry   pointer, embed, map        Foo { bar: u32 = 1 }    map[hash,u32] = { 0x1 = 1 }
    /// Value   list, list2, option        list[u32] = { 1, 2 }    option[u32] = { 1 }
    /// Value   vec2/3/4, rgba, mtx44      vec3 = { 1, 2, 3 }
    /// None    everything else            u32 = 1                 string = "a"
    /// ```
    pub fn item_shape(&self) -> Option<ItemShape> {
        use PropertyKind as K;
        Some(match self.base() {
            K::Struct | K::Embedded | K::Map => ItemShape::Entry,
            K::Container | K::UnorderedContainer | K::Optional => ItemShape::Value,
            // a listlike spells its components out as bare values, `vec3 = { 1, 2, 3 }`
            K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44 => ItemShape::Value,
            _ => return None,
        })
    }

    pub fn make_default(&self) -> PropertyValueEnum {
        let subtype = |n: usize| self.subtype(n).unwrap_or_default().kind();

        if let Some(hash_type) = self.hash_type() {
            return PropertyValueEnum::Hash(values::Hash::new(hash_type.from_hash(0)));
        }

        match self.base() {
            PropertyKind::Map => PropertyValueEnum::Map(
                values::Map::empty(subtype(0), subtype(1)).unwrap_or_default(),
            ),
            PropertyKind::UnorderedContainer => {
                PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(
                    values::Container::empty(subtype(0)).unwrap_or_default(),
                ))
            }
            PropertyKind::Container => PropertyValueEnum::Container(
                values::Container::empty(subtype(0)).unwrap_or_default(),
            ),
            PropertyKind::Optional => {
                PropertyValueEnum::Optional(values::Optional::empty(subtype(0)).unwrap_or_default())
            }

            PropertyKind::Hash => {
                unreachable!("earlier if let should avoid this");
            }

            kind => kind.default_value(),
        }
    }
}

/// The shape an item must have to sit inside a type's `{ .. }` block.
///
/// Which one a type wants is fixed by its base kind - see [`RitoType::item_shape`].
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum ItemShape {
    /// `key: type = value` (or `key = value`)
    Entry,
    /// a bare value
    Value,
}

impl Display for ItemShape {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ItemShape::Entry => "an entry ('name: type = value')",
            ItemShape::Value => "a value",
        })
    }
}

pub trait PropertyValueExt {
    /// Returns the type of the value. Each `hash` slot holds the type that
    /// [`hash_type_for_width`] returns for the width of its values.
    fn rito_type(&self) -> RitoType;
}
impl PropertyValueExt for PropertyValueEnum {
    fn rito_type(&self) -> RitoType {
        #[allow(
            clippy::unwrap_used,
            reason = "meta values should be safe by construction"
        )]
        match self {
            PropertyValueEnum::Hash(hash) => ScalarType::from(match hash.width() {
                HashWidth::W4 => HashType::DEFAULT,
                HashWidth::W8 => HashType::width_only(HashWidth::W8),
            })
            .into(),

            PropertyValueEnum::Map(map) => {
                let (key, value) = map
                    .entries()
                    .first()
                    .map(|(k, v)| {
                        (
                            k.rito_type().as_scalar().unwrap(),
                            v.rito_type().as_scalar().unwrap(),
                        )
                    })
                    .unwrap_or_else(|| {
                        (
                            ScalarType::try_from(map.key_kind()).unwrap(),
                            ScalarType::try_from(map.value_kind()).unwrap(),
                        )
                    });
                RitoType::map(key, value)
            }
            PropertyValueEnum::UnorderedContainer(container) => ContainerType::UnorderedContainer(
                container
                    .items()
                    .first()
                    .map(|v| v.rito_type().as_scalar().unwrap())
                    .or_else(|| ScalarType::try_from(container.item_kind()).ok()),
            )
            .into(),
            PropertyValueEnum::Container(container) => ContainerType::Container(
                container
                    .items()
                    .first()
                    .map(|v| v.rito_type().as_scalar().unwrap())
                    .or_else(|| ScalarType::try_from(container.item_kind()).ok()),
            )
            .into(),
            PropertyValueEnum::Optional(optional) => ContainerType::Optional(
                optional
                    .value()
                    .and_then(|v| v.rito_type().as_scalar())
                    .or_else(|| ScalarType::try_from_kind(optional.item_kind()).ok()),
            )
            .into(),

            #[allow(
                clippy::unwrap_used,
                reason = "all non scalar cases should be handled in the above match arms"
            )]
            _ => RitoType::try_from_scalar(self.kind()).unwrap(),
        }
    }
}
