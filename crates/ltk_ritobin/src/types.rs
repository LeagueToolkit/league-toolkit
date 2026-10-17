//! Type name mappings for ritobin format.

use ltk_hash::{HashAlgorithm, HashWidth, Hasher};
use ltk_meta::{property::values, PropertyKind, PropertyValueEnum};
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

/// Returns the ritobin name of a hash algorithm: `fnv1a`, `xxh64` or `xxh3`.
pub const fn hash_algorithm_name(algorithm: HashAlgorithm) -> &'static str {
    match algorithm {
        HashAlgorithm::Fnv1a32 => "fnv1a",
        HashAlgorithm::Xxh64 => "xxh64",
        HashAlgorithm::Xxh3 => "xxh3",
        _ => "unknown",
    }
}

/// Returns the hash algorithm of a ritobin name. Returns `None` if `name` is not `fnv1a`,
/// `xxh64` or `xxh3`.
pub fn hash_algorithm_from_name(name: &str) -> Option<HashAlgorithm> {
    match name {
        "fnv1a" => Some(HashAlgorithm::Fnv1a32),
        "xxh64" => Some(HashAlgorithm::Xxh64),
        "xxh3" => Some(HashAlgorithm::Xxh3),
        _ => None,
    }
}

/// Returns the ritobin name of a hash width: `u32` or `u64`.
pub const fn hash_width_name(width: HashWidth) -> &'static str {
    match width {
        HashWidth::W4 => "u32",
        HashWidth::W8 => "u64",
    }
}

/// Returns the hash width of a ritobin name. Returns `None` if `name` is not `u32` or `u64`.
pub fn hash_width_from_name(name: &str) -> Option<HashWidth> {
    match name {
        "u32" => Some(HashWidth::W4),
        "u64" => Some(HashWidth::W8),
        _ => None,
    }
}

/// Returns the hasher of the type `hash[algorithm,width]`.
///
/// The hasher lowercases the string. Every hasher in the class dumps of 16.21 lowercases the
/// string.
pub const fn rito_hasher(algorithm: HashAlgorithm, width: HashWidth) -> Hasher {
    Hasher {
        width,
        algorithm,
        lowercased: true,
    }
}

/// Returns the hasher that the printer writes for a `hash` value of `width`.
///
/// A bin file does not store the hash function. Returns [`Hasher::DEFAULT`] for width 4 and
/// XXH3 for width 8. Every 8-byte hasher in the class dumps of 16.21 is XXH3.
pub const fn hasher_for_width(width: HashWidth) -> Hasher {
    match width {
        HashWidth::W4 => Hasher::DEFAULT,
        HashWidth::W8 => rito_hasher(HashAlgorithm::Xxh3, HashWidth::W8),
    }
}

/// Ritobin type representation for parsing.
///
/// The type `hash[algorithm,width]` gives the hash function and the stored width of a `hash`,
/// for example `hash[xxh3,u64]`. The type `hash` is `hash[fnv1a,u32]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitoType {
    pub base: PropertyKind,
    pub subtypes: [Option<PropertyKind>; 2],
    /// The hashers of the base and of the two subtypes, in that order. A slot that is not
    /// `hash` holds [`Hasher::DEFAULT`].
    pub hashers: [Hasher; 3],
}

impl From<PropertyKind> for RitoType {
    fn from(kind: PropertyKind) -> Self {
        Self::simple(kind)
    }
}

/// Writes the ritobin name of `kind`. Writes `hash[algorithm,width]` if `kind` is `hash` and
/// `hasher` is not [`Hasher::DEFAULT`].
fn fmt_kind(
    f: &mut std::fmt::Formatter<'_>,
    kind: PropertyKind,
    hasher: Hasher,
) -> std::fmt::Result {
    f.write_str(kind.to_rito_name())?;
    if kind == PropertyKind::Hash && hasher != Hasher::DEFAULT {
        write!(
            f,
            "[{},{}]",
            hash_algorithm_name(hasher.algorithm),
            hash_width_name(hasher.width)
        )?;
    }
    Ok(())
}

impl Display for RitoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_kind(f, self.base, self.hashers[0])?;

        match self.subtypes {
            [None, None] => Ok(()),
            [Some(a), None] => {
                f.write_str("[")?;
                fmt_kind(f, a, self.hashers[1])?;
                f.write_str("]")
            }
            [Some(a), Some(b)] => {
                f.write_str("[")?;
                fmt_kind(f, a, self.hashers[1])?;
                f.write_str(",")?;
                fmt_kind(f, b, self.hashers[2])?;
                f.write_str("]")
            }
            _ => f.write_str("[!!]"),
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
        Self::new(kind, [None, None])
    }

    /// Returns the type `hash` with `hasher`.
    pub const fn hash(hasher: Hasher) -> Self {
        let mut rito = Self::simple(PropertyKind::Hash);
        rito.hashers[0] = hasher;
        rito
    }

    pub const fn single(base: PropertyKind, sub: PropertyKind) -> Option<Self> {
        if base.subtype_count() != 1 {
            return None;
        }
        Some(Self::new(base, [Some(sub), None]))
    }

    /// Returns the type with [`Hasher::DEFAULT`] in each slot.
    pub const fn new(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Self {
        Self {
            base,
            subtypes,
            hashers: [Hasher::DEFAULT; 3],
        }
    }

    /// Returns the hasher of the base. Returns [`Hasher::DEFAULT`] if the base is not `hash`.
    pub const fn hasher(&self) -> Hasher {
        self.hashers[0]
    }

    /// Returns the type of subtype `idx`, with the hasher of the subtype. Returns `none` if the
    /// subtype is missing.
    pub const fn item(&self, idx: usize) -> Self {
        let kind = match self.subtypes[idx] {
            Some(kind) => kind,
            None => PropertyKind::None,
        };
        let mut rito = Self::simple(kind);
        rito.hashers[0] = self.hashers[idx + 1];
        rito
    }

    /// Returns the type with [`Hasher::DEFAULT`] in each slot.
    pub const fn without_hashers(self) -> Self {
        Self::new(self.base, self.subtypes)
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
                    Ok(RitoType::new(base, [Some(sub), None]))
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
                    Ok(RitoType::new(base, subtypes))
                }
            },
            _ => unreachable!(),
        }
    }

    pub fn container(value: PropertyKind) -> Self {
        Self::new(PropertyKind::Container, [Some(value), None])
    }

    pub fn map(key: PropertyKind, value: PropertyKind) -> Self {
        Self::new(PropertyKind::Map, [Some(key), Some(value)])
    }

    pub fn subtype(&self, idx: usize) -> PropertyKind {
        self.subtypes[idx].unwrap_or_default()
    }

    pub fn value_subtype(&self) -> Option<PropertyKind> {
        self.subtypes[1].or(self.subtypes[0])
    }

    /// Returns the type of [`Self::value_subtype`], with its hasher.
    pub fn value_item(&self) -> Option<Self> {
        match self.subtypes {
            [_, Some(_)] => Some(self.item(1)),
            [Some(_), None] => Some(self.item(0)),
            [None, None] => None,
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
        Some(match self.base {
            K::Struct | K::Embedded | K::Map => ItemShape::Entry,
            K::Container | K::UnorderedContainer | K::Optional => ItemShape::Value,
            // a listlike spells its components out as bare values, `vec3 = { 1, 2, 3 }`
            K::Vector2 | K::Vector3 | K::Vector4 | K::Color | K::Matrix44 => ItemShape::Value,
            _ => return None,
        })
    }

    pub fn make_default<M: Default>(&self) -> PropertyValueEnum {
        match self.base {
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
            PropertyKind::Hash => {
                PropertyValueEnum::Hash(values::Hash::new(self.hasher().from_hash(0)))
            }

            _ => self.base.default_value(),
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
    /// Returns the type of the value. Each `hash` slot holds the hasher that
    /// [`hasher_for_width`] returns for the width of its values.
    fn rito_type(&self) -> RitoType;
}
impl PropertyValueExt for PropertyValueEnum {
    fn rito_type(&self) -> RitoType {
        let hasher = |width: Option<HashWidth>| hasher_for_width(width.unwrap_or_default());
        match self {
            PropertyValueEnum::Hash(hash) => RitoType::hash(hasher(Some(hash.width()))),
            PropertyValueEnum::Map(map) => {
                let mut rito = RitoType::map(map.key_kind(), map.value_kind());
                rito.hashers[1] = hasher(map.key_hash_width());
                rito.hashers[2] = hasher(map.value_hash_width());
                rito
            }
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container))
            | PropertyValueEnum::Container(container) => {
                let mut rito = RitoType::new(self.kind(), [Some(container.item_kind()), None]);
                rito.hashers[1] = hasher(container.hash_width());
                rito
            }
            PropertyValueEnum::Optional(optional) => {
                let mut rito = RitoType::new(self.kind(), [Some(optional.item_kind()), None]);
                if let Some(PropertyValueEnum::Hash(hash)) = optional.value() {
                    rito.hashers[1] = hasher(Some(hash.width()));
                }
                rito
            }

            _ => RitoType::simple(self.kind()),
        }
    }
}
