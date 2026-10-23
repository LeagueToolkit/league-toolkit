//! Type name mappings for ritobin format.

use ltk_hash::HashWidth;
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

#[cfg(test)]
mod tests;

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

/// Returns the [`RitoType`] named by [`ltk_meta::PropertyKind`] variants, at compile time.
///
/// `rito!(U32)`, `rito!(Container[U32])` and `rito!(Map[Hash, Embedded])` are the three forms.
/// A type that does not build fails to compile. Each `hash` holds [`HashType::DEFAULT`].
#[macro_export]
macro_rules! rito {
    ($kind:ident) => {
        const {
            match $crate::RitoType::try_from_scalar($crate::__private::PropertyKind::$kind) {
                Ok(t) => t,
                Err(_) => panic!("invalid simple rito type"),
            }
        }
    };
    ($kind:ident [ $sub:ident ]) => {
        const {
            match $crate::RitoType::try_from_parts(
                $crate::__private::PropertyKind::$kind,
                [Some($crate::__private::PropertyKind::$sub), None],
            ) {
                Ok(t) => t,
                Err(_) => panic!("invalid rito type"),
            }
        }
    };
    ($kind:ident [ $a:ident, $b:ident ]) => {
        const {
            match $crate::RitoType::try_from_parts(
                $crate::__private::PropertyKind::$kind,
                [
                    Some($crate::__private::PropertyKind::$a),
                    Some($crate::__private::PropertyKind::$b),
                ],
            ) {
                Ok(t) => t,
                Err(_) => panic!("invalid rito type"),
            }
        }
    };
}

impl RitoType {
    /// Returns the type of `kind`, without subtypes.
    ///
    /// A container kind has no subtypes. A `hash` holds [`HashType::DEFAULT`].
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
            PropertyKind::UnorderedContainer => Self::Container(C::OrderedList(None)),
            PropertyKind::Optional => Self::Container(C::Optional(None)),
            PropertyKind::Map => Self::Container(C::Map(None)),
        }
    }
}

impl RitoType {
    /// Returns the scalar type. Returns `None` for a container.
    pub const fn as_scalar(self) -> Option<ScalarType> {
        match self {
            RitoType::Scalar(kind) => Some(kind),
            _ => None,
        }
    }

    /// Returns the container type. Returns `None` for a scalar.
    pub const fn as_container(self) -> Option<ContainerType> {
        match self {
            RitoType::Container(kind) => Some(kind),
            _ => None,
        }
    }

    /// Returns whether `self` and `other` are the same type.
    ///
    /// A container subtype that is `None` on either side matches any subtype. Hash types compare
    /// in full.
    pub fn equivalent_to(&self, other: &Self) -> bool {
        match (self, other) {
            (RitoType::Scalar(a), RitoType::Scalar(b)) => a == b,
            (RitoType::Container(a), RitoType::Container(b)) => a.equivalent_to(b),
            _ => false,
        }
    }

    /// Returns the scalar type of `kind`.
    ///
    /// # Errors
    /// Returns [`BadScalarType`] if `kind` is a container kind.
    #[inline(always)]
    pub const fn try_from_scalar(kind: PropertyKind) -> Result<Self, BadScalarType> {
        match ScalarType::try_from_kind(kind) {
            Ok(v) => Ok(Self::Scalar(v)),
            Err(e) => Err(e),
        }
    }

    /// Returns the hash type of a `hash`. Returns `None` for any other type.
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

    /// Returns the type with `hash_types` in its `hash` subtypes.
    ///
    /// The first subtype takes `hash_types[0]`. A map value takes `hash_types[1]`. A scalar is
    /// returned unchanged.
    pub fn with_subtype_hash_types(self, hash_types: [HashType; 2]) -> Self {
        match self {
            RitoType::Scalar(_) => self,
            RitoType::Container(container) => {
                Self::Container(container.with_hash_types(hash_types))
            }
        }
    }

    /// Returns the type of `base` with `subtypes`.
    ///
    /// A type with no subtypes is a scalar. Each `hash` holds [`HashType::DEFAULT`].
    ///
    /// # Errors
    /// Returns [`BadRitoType`] if `base` is a container kind and no subtypes are given, or if
    /// the subtypes do not build a container type of `base`.
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

    /// Returns the type of `base` with `subtypes`.
    ///
    /// A subtype that does not fit `base` is left out. Each `hash` holds [`HashType::DEFAULT`].
    pub fn from_parts(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Self {
        match ContainerType::from_parts(base, subtypes) {
            Some(container) => container.into(),
            None => Self::from_property_kind(base),
        }
    }

    /// Returns the type `list[item]`.
    pub const fn list(item: ScalarType) -> Self {
        Self::Container(ContainerType::Container(Some(item)))
    }

    /// Returns the type `map[key,value]`.
    pub const fn map(key: ScalarType, value: ScalarType) -> Self {
        Self::Container(ContainerType::Map(Some((key, value))))
    }

    /// Returns the [`PropertyKind`] of the type. A container returns its kind without subtypes.
    pub const fn base(&self) -> PropertyKind {
        match self {
            RitoType::Scalar(scalar_type) => scalar_type.kind(),
            RitoType::Container(container_type) => container_type.base_kind(),
        }
    }

    /// Returns the value type of a map, or the item type of a `list`, `list2` or `option`.
    ///
    /// Returns `None` for a scalar, and for a container that does not give the subtype.
    pub const fn value_subtype(&self) -> Option<ScalarType> {
        match self {
            RitoType::Scalar(_) => None,
            RitoType::Container(container) => match container {
                ContainerType::Map(types) => match types {
                    Some((_, value)) => Some(*value),
                    None => None,
                },
                ContainerType::Container(value) => *value,
                ContainerType::OrderedList(value) => *value,
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

    /// Returns the default value of the type.
    ///
    /// A `hash` is zero at the width of its hash type. A container is empty.
    pub fn make_default(&self) -> PropertyValueEnum {
        use ContainerType as C;
        use PropertyValueEnum as P;
        let kind = |item: Option<ScalarType>| item.unwrap_or_default().kind();

        match *self {
            RitoType::Scalar(ScalarType::Hash(hash_type)) => {
                P::Hash(values::Hash::new(hash_type.from_hash(0)))
            }
            RitoType::Scalar(scalar) => scalar.kind().default_value(),
            RitoType::Container(C::Map(types)) => {
                let (key, value) = types.unwrap_or_default();
                P::Map(values::Map::empty(key.kind(), value.kind()).unwrap_or_default())
            }
            RitoType::Container(C::Container(item)) => {
                P::Container(values::Container::empty(kind(item)).unwrap_or_default())
            }
            RitoType::Container(C::OrderedList(item)) => {
                P::UnorderedContainer(values::UnorderedContainer(
                    values::Container::empty(kind(item)).unwrap_or_default(),
                ))
            }
            RitoType::Container(C::Optional(item)) => {
                P::Optional(values::Optional::empty(kind(item)).unwrap_or_default())
            }
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

/// Extension trait for reading the ritobin type of a [`PropertyValueEnum`].
pub trait PropertyValueExt {
    /// Returns the type of the value. Each `hash` slot holds the type that
    /// [`hash_type_for_width`] returns for the width of its values.
    fn rito_type(&self) -> RitoType;
}
impl PropertyValueExt for PropertyValueEnum {
    fn rito_type(&self) -> RitoType {
        let item = |kind: PropertyKind, width: Option<HashWidth>| {
            ScalarType::try_from_kind(kind)
                .ok()
                .map(|item| item.with_hash_type(hash_type_for_width(width.unwrap_or_default())))
        };
        match self {
            PropertyValueEnum::Hash(hash) => {
                ScalarType::Hash(hash_type_for_width(hash.width())).into()
            }
            PropertyValueEnum::Map(map) => ContainerType::Map(
                item(map.key_kind(), map.key_hash_width())
                    .zip(item(map.value_kind(), map.value_hash_width())),
            )
            .into(),
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container)) => {
                ContainerType::OrderedList(item(container.item_kind(), container.hash_width()))
                    .into()
            }
            PropertyValueEnum::Container(container) => {
                ContainerType::Container(item(container.item_kind(), container.hash_width())).into()
            }
            PropertyValueEnum::Optional(optional) => {
                let width = match optional.value() {
                    Some(PropertyValueEnum::Hash(hash)) => Some(hash.width()),
                    _ => None,
                };
                ContainerType::Optional(item(optional.item_kind(), width)).into()
            }
            _ => RitoType::from_property_kind(self.kind()),
        }
    }
}
