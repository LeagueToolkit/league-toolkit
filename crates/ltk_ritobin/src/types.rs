//! Type name mappings for ritobin format.

use ltk_meta::{property::values, traits::PropertyExt, PropertyKind, PropertyValueEnum};
use std::fmt::Display;

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

/// Ritobin type representation for parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitoType {
    pub base: PropertyKind,
    pub subtypes: [Option<PropertyKind>; 2],
}

impl Display for RitoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let base = self.base.to_rito_name();

        match self.subtypes {
            [None, None] => f.write_str(base),
            [Some(a), None] => write!(f, "{base}[{}]", a.to_rito_name()),
            [Some(a), Some(b)] => {
                write!(f, "{base}[{},{}]", a.to_rito_name(), b.to_rito_name())
            }
            _ => write!(f, "{base}[!!]"),
        }
    }
}

#[macro_export]
macro_rules! rito {
    ($kind:ident) => {
        const {
            match RitoType::try_new(ltk_meta::PropertyKind::$kind, [None, None]) {
                Ok(t) => t,
                Err(_) => panic!("invalid simple rito type"),
            }
        }
    };
    ($kind:ident [ $sub:ident ]) => {
        const {
            match RitoType::try_new(
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
            match RitoType::try_new(
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

#[derive(thiserror::Error, Debug)]
pub enum ConstructError {
    #[error("Got {got} subtypes, expected {expected}")]
    BadSubtypeCount { got: u8, expected: u8 },
    #[error("{got:?} is not primitive - containers can only hold primitive types")]
    NonPrimitiveContainer { got: PropertyKind },
    #[error("{got:?} is not a valid map key")]
    BadMapKey { got: PropertyKind },
    #[error("{got:?} is not a valid map value")]
    BadMapValue { got: PropertyKind },
}

impl RitoType {
    pub const fn simple(kind: PropertyKind) -> Self {
        Self {
            base: kind,
            subtypes: [None, None],
        }
    }

    pub const fn single(base: PropertyKind, sub: PropertyKind) -> Option<Self> {
        if base.subtype_count() != 1 {
            return None;
        }
        Some(Self {
            base,
            subtypes: [Some(sub), None],
        })
    }
    pub const fn new(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Self {
        Self { base, subtypes }
    }

    pub const fn try_new(
        base: PropertyKind,
        subtypes: [Option<PropertyKind>; 2],
    ) -> Result<Self, ConstructError> {
        use ConstructError::*;

        match base.subtype_count() {
            0 => match subtypes {
                [None, None] => Ok(Self::simple(base)),
                [Some(_), None] | [None, Some(_)] => Err(BadSubtypeCount {
                    got: 1,
                    expected: 0,
                }),
                [Some(_), Some(_)] => Err(BadSubtypeCount {
                    got: 2,
                    expected: 0,
                }),
            },
            1 => match subtypes {
                [Some(sub), None] | [None, Some(sub)] => {
                    if !sub.is_primitive() {
                        return Err(NonPrimitiveContainer { got: sub });
                    }
                    Ok(RitoType {
                        base,
                        subtypes: [Some(sub), None],
                    })
                }
                [None, None] => Err(BadSubtypeCount {
                    got: 0,
                    expected: 1,
                }),
                [Some(_), Some(_)] => Err(BadSubtypeCount {
                    got: 2,
                    expected: 1,
                }),
            },
            2 => match subtypes {
                [None, None] => Err(BadSubtypeCount {
                    got: 0,
                    expected: 2,
                }),
                [Some(_), None] | [None, Some(_)] => Err(BadSubtypeCount {
                    got: 1,
                    expected: 2,
                }),
                [Some(a), Some(b)] => {
                    if !a.is_valid_map_key() {
                        return Err(BadMapKey { got: a });
                    }
                    if b.is_container() {
                        return Err(BadMapValue { got: b });
                    }
                    Ok(RitoType { base, subtypes })
                }
            },
            _ => unreachable!(),
        }
    }

    pub fn container(value: PropertyKind) -> Self {
        Self {
            base: PropertyKind::Container,
            subtypes: [Some(value), None],
        }
    }

    pub fn map(key: PropertyKind, value: PropertyKind) -> Self {
        Self {
            base: PropertyKind::Map,
            subtypes: [Some(key), Some(value)],
        }
    }

    pub fn subtype(&self, idx: usize) -> PropertyKind {
        self.subtypes[idx].unwrap_or_default()
    }

    pub fn value_subtype(&self) -> Option<PropertyKind> {
        self.subtypes[1].or(self.subtypes[0])
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
        Some(match self.base {
            K::Struct | K::Embedded | K::Map => ItemShape::Entry,
            K::Container | K::UnorderedContainer | K::Optional => ItemShape::Value,
            // a listlike spells its components out as bare values, `vec3 = { 1, 2, 3 }`
            K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44 => ItemShape::Value,
            _ => return None,
        })
    }

    pub fn make_default<M: Default>(&self, span: M) -> PropertyValueEnum<M> {
        let mut value = match self.base {
            PropertyKind::Map => PropertyValueEnum::Map(
                values::Map::empty(self.subtype(0), self.subtype(1)).unwrap_or_default(),
            ),
            PropertyKind::UnorderedContainer => {
                PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(
                    values::Container::empty(self.subtype(0)).unwrap_or_default(),
                ))
            }
            PropertyKind::Container => PropertyValueEnum::Container(
                values::Container::empty(self.subtype(0)).unwrap_or_default(),
            ),
            PropertyKind::Optional => PropertyValueEnum::Optional(
                values::Optional::empty(self.subtype(0)).unwrap_or_default(),
            ),

            _ => self.base.default_value(),
        };
        *value.meta_mut() = span;
        value
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
    fn rito_type(&self) -> RitoType;
}
impl<M> PropertyValueExt for PropertyValueEnum<M> {
    fn rito_type(&self) -> RitoType {
        let base = self.kind();
        let subtypes = match self {
            PropertyValueEnum::Map(map) => [Some(map.key_kind()), Some(map.value_kind())],
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container))
            | PropertyValueEnum::Container(container) => [Some(container.item_kind()), None],
            PropertyValueEnum::Optional(optional) => [Some(optional.item_kind()), None],

            _ => [None, None],
        };
        RitoType { base, subtypes }
    }
}
