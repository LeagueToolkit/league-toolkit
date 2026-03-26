# Public API Contract: ltk_inibin + ltk_inibin_names

**Phase**: 1 | **Date**: 2026-03-25 | **Updated**: 2026-03-26

## ltk_hash additions

### `ltk_hash::sdbm`

```rust
/// Compute SDBM hash of a lowercased string.
pub fn hash_lower(input: impl AsRef<str>) -> u32;

/// Compute SDBM hash of two strings joined by a delimiter, all lowercased.
/// Used for inibin keys: hash_lower_with_delimiter(section, property, '*')
pub fn hash_lower_with_delimiter(a: impl AsRef<str>, b: impl AsRef<str>, delimiter: char) -> u32;
```

## ltk_inibin public API

### Inibin (formerly InibinFile)

```rust
/// Top-level inibin/troybin file container.
pub struct Inibin { /* bucket-based internal storage */ }

impl Inibin {
    /// Create an empty inibin file.
    pub fn new() -> Self;

    /// Parse an inibin file from a seekable reader.
    /// Supports version 1 (legacy) and version 2.
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self>;

    /// Write as version 2 inibin format.
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// Get a value by hash key, searching all buckets.
    /// Returns None if key not found.
    pub fn get(&self, key: u32) -> Option<&Value>;

    /// Get a typed value by hash key.
    pub fn get_as<'a, T: FromValue<'a>>(&'a self, key: u32) -> Option<T>;

    /// Get a typed value with default.
    pub fn get_or<'a, T: FromValue<'a>>(&'a self, key: u32, default: T) -> T;

    /// Insert or update a value. Routes to the correct bucket by value type.
    /// If the key exists in a different-type bucket, removes it first.
    pub fn insert(&mut self, key: u32, value: impl Into<Value>);

    /// Remove a value by hash key from all buckets.
    /// Returns the removed value if found.
    pub fn remove(&mut self, key: u32) -> Option<Value>;

    /// Check if a key exists in any bucket.
    pub fn contains_key(&self, key: u32) -> bool;

    /// Total entry count across all sections.
    pub fn len(&self) -> usize;

    /// Whether the inibin has no entries.
    pub fn is_empty(&self) -> bool;

    /// Iterate over all key-value pairs across all buckets.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &Value)>;

    /// Get a reference to a specific section by flag type.
    pub fn section(&self, flags: ValueFlags) -> Option<&Section>;

    /// Get a mutable reference to a specific section by flag type.
    pub fn section_mut(&mut self, flags: ValueFlags) -> Option<&mut Section>;
}
```

### Section (formerly InibinSet)

```rust
/// A typed bucket of key-value pairs.
pub struct Section { /* properties: IndexMap<u32, Value> */ }

impl Section {
    /// Get value by hash key within this section.
    pub fn get(&self, key: u32) -> Option<&Value>;

    /// Insert a key-value pair into this section.
    pub fn insert(&mut self, key: u32, value: Value);

    /// Remove a key-value pair from this section.
    pub fn remove(&mut self, key: u32) -> Option<Value>;

    /// Number of entries in this section.
    pub fn len(&self) -> usize;

    /// Whether this section is empty.
    pub fn is_empty(&self) -> bool;

    /// The flag type of this section.
    pub fn kind(&self) -> ValueFlags;

    /// Iterate over hash keys in this section.
    pub fn keys(&self) -> impl Iterator<Item = &u32>;

    /// Iterate over values in this section.
    pub fn values(&self) -> impl Iterator<Item = &Value>;

    /// Iterate over key-value pairs in this section.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &Value)>;
}
```

### ValueFlags (formerly InibinFlags)

```rust
bitflags! {
    /// Bitfield representing inibin value set types.
    pub struct ValueFlags: u16 {
        const INT32_LIST    = 1 << 0;
        const F32_LIST      = 1 << 1;
        const U8_LIST       = 1 << 2;
        const INT16_LIST    = 1 << 3;
        const INT8_LIST     = 1 << 4;
        const BIT_LIST      = 1 << 5;
        const VEC3_U8_LIST  = 1 << 6;
        const VEC3_F32_LIST = 1 << 7;
        const VEC2_U8_LIST  = 1 << 8;
        const VEC2_F32_LIST = 1 << 9;
        const VEC4_U8_LIST  = 1 << 10;
        const VEC4_F32_LIST = 1 << 11;
        const STRING_LIST   = 1 << 12;
        const INT64_LIST    = 1 << 13;
    }
}
```

### Value (formerly InibinValue)

```rust
/// Typed value stored in an inibin section.
pub enum Value {
    I32(i32),
    F32(f32),
    U8(u8),        // Raw byte; use as_f32() for packed float conversion
    I16(i16),
    I8(u8),
    Bool(bool),
    Vec3U8([u8; 3]),  // Raw bytes; use as_vec3() for packed float conversion
    Vec3F32(Vec3),
    Vec2U8([u8; 2]),  // Raw bytes; use as_vec2() for packed float conversion
    Vec2F32(Vec2),
    Vec4U8([u8; 4]),  // Raw bytes; use as_vec4() for packed float conversion
    Vec4F32(Vec4),
    String(String),
    I64(i64),
}

impl Value {
    /// Returns the ValueFlags variant this value belongs to.
    pub fn flags(&self) -> ValueFlags;

    /// Returns the value as f32, handling both F32 and packed U8 variants.
    pub fn as_f32(&self) -> Option<f32>;

    /// Returns the value as Vec2, handling both Vec2F32 and packed Vec2U8 variants.
    pub fn as_vec2(&self) -> Option<Vec2>;

    /// Returns the value as Vec3, handling both Vec3F32 and packed Vec3U8 variants.
    pub fn as_vec3(&self) -> Option<Vec3>;

    /// Returns the value as Vec4, handling both Vec4F32 and packed Vec4U8 variants.
    pub fn as_vec4(&self) -> Option<Vec4>;
}
```

### Error types

```rust
#[derive(Debug, thiserror::Error)]
pub enum InibinError {
    #[error("unsupported inibin version: {0}")]
    UnsupportedVersion(u8),

    #[error("u8 float overflow: {0} is outside range 0.0-25.5")]
    U8FloatOverflow(f32),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = core::result::Result<T, InibinError>;
```

### Crate dependencies

```toml
[dependencies]
thiserror = { workspace = true }
byteorder = { workspace = true }
bitflags = { workspace = true }
glam = { workspace = true }
ltk_io_ext = { version = "0.4.1", path = "../ltk_io_ext" }
ltk_hash = { version = "0.2.5", path = "../ltk_hash/" }

[dev-dependencies]
approx = { workspace = true }
```

## ltk_inibin_names public API

### Lookup function

```rust
/// Look up the human-readable (section, name) pair for an inibin hash key.
/// Returns `None` if the hash is not in the known fixlist.
pub fn lookup(hash: u32) -> Option<(&'static str, &'static str)>;
```

### Crate dependencies

```toml
[dependencies]
phf = { workspace = true }

[build-dependencies]
phf_codegen = { workspace = true }
```
