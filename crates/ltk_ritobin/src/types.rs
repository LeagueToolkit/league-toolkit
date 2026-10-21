//! Type name mappings for ritobin format.

use ltk_hash::{HashAlgorithm, HashValue, HashWidth, Hasher};
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

/// Returns the hasher of the type `hash[width,algorithm]`.
///
/// The hasher lowercases the string. Every hasher in the class dumps of 16.21 lowercases the
/// string.
pub const fn rito_hasher(width: HashWidth, algorithm: HashAlgorithm) -> Hasher {
    Hasher {
        width,
        algorithm,
        lowercased: true,
    }
}

/// The type arguments of `hash`: the stored width and the hash function.
///
/// `hash` is `hash[u32,fnv1a]`. `hash[u32]` and `hash[u64]` give the width alone, for a value
/// whose hash function is not known: a hex literal takes that width, and a string is a type
/// mismatch because no function hashes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, std::hash::Hash)]
pub struct HashType {
    /// The stored width.
    pub width: HashWidth,
    /// The hash function. `None` for `hash[u32]` and `hash[u64]`.
    pub algorithm: Option<HashAlgorithm>,
}

impl HashType {
    /// The type of plain `hash`: 4 bytes, FNV-1a over the lowercased string.
    pub const DEFAULT: Self = Self::new(HashWidth::W4, HashAlgorithm::Fnv1a32);

    /// Returns the type `hash[width,algorithm]`.
    pub const fn new(width: HashWidth, algorithm: HashAlgorithm) -> Self {
        Self {
            width,
            algorithm: Some(algorithm),
        }
    }

    /// Returns the type `hash[width]`, whose hash function is not known.
    pub const fn width_only(width: HashWidth) -> Self {
        Self {
            width,
            algorithm: None,
        }
    }

    /// Returns the hasher of the type. Returns `None` if the hash function is not known.
    pub const fn hasher(self) -> Option<Hasher> {
        match self.algorithm {
            Some(algorithm) => Some(rito_hasher(self.width, algorithm)),
            None => None,
        }
    }

    /// Returns `hash` at the width of the type. Keeps only the low 32 bits if the width is 4.
    pub const fn from_hash(self, hash: u64) -> HashValue {
        HashValue::with_width(hash, self.width)
    }

    /// Returns the hash of `text` at the width of the type. Returns `None` if the hash function
    /// is not known.
    pub fn hash_str(self, text: &str) -> Option<HashValue> {
        Some(self.hasher()?.hash_str(text))
    }
}

impl Default for HashType {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Writes `hash`, `hash[width]` or `hash[width,algorithm]`. Writes `hash` for
/// [`HashType::DEFAULT`].
impl Display for HashType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(PropertyKind::Hash.to_rito_name())?;
        if *self == Self::DEFAULT {
            return Ok(());
        }
        write!(f, "[{}", hash_width_name(self.width))?;
        if let Some(algorithm) = self.algorithm {
            write!(f, ",{}", hash_algorithm_name(algorithm))?;
        }
        f.write_str("]")
    }
}

/// Returns the type that the printer writes for a `hash` value of `width`.
///
/// A bin file does not store the hash function. Returns [`HashType::DEFAULT`] for width 4, as
/// plain `hash` is read as FNV-1a. Returns `hash[u64]` for width 8.
pub const fn hash_type_for_width(width: HashWidth) -> HashType {
    match width {
        HashWidth::W4 => HashType::DEFAULT,
        HashWidth::W8 => HashType::width_only(HashWidth::W8),
    }
}

/// Ritobin type representation for parsing.
///
/// The type `hash[width,algorithm]` gives the stored width and the hash function of a `hash`,
/// for example `hash[u64,xxh3]`. The type `hash` is `hash[u32,fnv1a]`. The type `hash[width]`
/// gives the width alone; see [`HashType`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RitoType {
    pub base: PropertyKind,
    pub subtypes: [Option<PropertyKind>; 2],
    /// The hash types of the base and of the two subtypes, in that order. A slot that is not
    /// `hash` holds [`HashType::DEFAULT`].
    pub hash_types: [HashType; 3],
}

impl From<PropertyKind> for RitoType {
    fn from(kind: PropertyKind) -> Self {
        Self::simple(kind)
    }
}

/// Writes the ritobin name of `kind`. Writes `hash_type` if `kind` is `hash`.
fn fmt_kind(
    f: &mut std::fmt::Formatter<'_>,
    kind: PropertyKind,
    hash_type: HashType,
) -> std::fmt::Result {
    if kind == PropertyKind::Hash {
        return hash_type.fmt(f);
    }
    f.write_str(kind.to_rito_name())
}

impl Display for RitoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt_kind(f, self.base, self.hash_types[0])?;

        match self.subtypes {
            [None, None] => Ok(()),
            [Some(a), None] => {
                f.write_str("[")?;
                fmt_kind(f, a, self.hash_types[1])?;
                f.write_str("]")
            }
            [Some(a), Some(b)] => {
                f.write_str("[")?;
                fmt_kind(f, a, self.hash_types[1])?;
                f.write_str(",")?;
                fmt_kind(f, b, self.hash_types[2])?;
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

    /// Returns the type `hash` with `hash_type`.
    pub const fn hash(hash_type: HashType) -> Self {
        let mut rito = Self::simple(PropertyKind::Hash);
        rito.hash_types[0] = hash_type;
        rito
    }

    pub const fn single(base: PropertyKind, sub: PropertyKind) -> Option<Self> {
        if base.subtype_count() != 1 {
            return None;
        }
        Some(Self::new(base, [Some(sub), None]))
    }

    /// Returns the type with [`HashType::DEFAULT`] in each slot.
    pub const fn new(base: PropertyKind, subtypes: [Option<PropertyKind>; 2]) -> Self {
        Self {
            base,
            subtypes,
            hash_types: [HashType::DEFAULT; 3],
        }
    }

    /// Returns the hash type of the base. Returns [`HashType::DEFAULT`] if the base is not
    /// `hash`.
    pub const fn hash_type(&self) -> HashType {
        self.hash_types[0]
    }

    /// Returns the type of subtype `idx`, with the hash type of the subtype. Returns `none` if
    /// the subtype is missing.
    pub const fn item(&self, idx: usize) -> Self {
        let kind = match self.subtypes[idx] {
            Some(kind) => kind,
            None => PropertyKind::None,
        };
        let mut rito = Self::simple(kind);
        rito.hash_types[0] = self.hash_types[idx + 1];
        rito
    }

    /// Returns the type with [`HashType::DEFAULT`] in each slot.
    pub const fn without_hash_types(self) -> Self {
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

    /// Returns the type of [`Self::value_subtype`], with its hash type.
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
                PropertyValueEnum::Hash(values::Hash::new(self.hash_type().from_hash(0)))
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
    /// Returns the type of the value. Each `hash` slot holds the type that
    /// [`hash_type_for_width`] returns for the width of its values.
    fn rito_type(&self) -> RitoType;
}
impl PropertyValueExt for PropertyValueEnum {
    fn rito_type(&self) -> RitoType {
        let hash_type = |width: Option<HashWidth>| hash_type_for_width(width.unwrap_or_default());
        match self {
            PropertyValueEnum::Hash(hash) => RitoType::hash(hash_type(Some(hash.width()))),
            PropertyValueEnum::Map(map) => {
                let mut rito = RitoType::map(map.key_kind(), map.value_kind());
                rito.hash_types[1] = hash_type(map.key_hash_width());
                rito.hash_types[2] = hash_type(map.value_hash_width());
                rito
            }
            PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container))
            | PropertyValueEnum::Container(container) => {
                let mut rito = RitoType::new(self.kind(), [Some(container.item_kind()), None]);
                rito.hash_types[1] = hash_type(container.hash_width());
                rito
            }
            PropertyValueEnum::Optional(optional) => {
                let mut rito = RitoType::new(self.kind(), [Some(optional.item_kind()), None]);
                if let Some(PropertyValueEnum::Hash(hash)) = optional.value() {
                    rito.hash_types[1] = hash_type(Some(hash.width()));
                }
                rito
            }

            _ => RitoType::simple(self.kind()),
        }
    }
}
