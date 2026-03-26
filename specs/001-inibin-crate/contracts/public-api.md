# Public API Contract: ltk_inibin + ltk_inibin_names

**Phase**: 1 | **Date**: 2026-03-25 | **Updated**: 2026-03-25

## ltk_hash additions

### `ltk_hash::sdbm`

```rust
/// Compute SDBM hash of a lowercased string.
pub fn hash_lower(input: &str) -> u32;

/// Compute SDBM hash of two strings joined by a delimiter, all lowercased.
/// Used for inibin keys: hash_lower_with_delimiter(section, property, '*')
pub fn hash_lower_with_delimiter(a: &str, b: &str, delimiter: char) -> u32;
```

## ltk_inibin public API

### InibinFile

```rust
/// Top-level inibin/troybin file container.
pub struct InibinFile { /* bucket-based internal storage */ }

impl InibinFile {
    /// Create an empty inibin file.
    pub fn new() -> Self;

    /// Parse an inibin file from a seekable reader.
    /// Supports version 1 (legacy) and version 2.
    pub fn from_reader<R: Read + Seek>(reader: &mut R) -> Result<Self>;

    /// Write as version 2 inibin format.
    pub fn to_writer<W: Write>(&self, writer: &mut W) -> Result<()>;

    /// Get a value by hash key, searching all buckets.
    /// Returns None if key not found.
    pub fn get(&self, key: u32) -> Option<&InibinValue>;

    /// Insert or update a value. Routes to the correct bucket by value type.
    /// If the key exists in a different-type bucket, removes it first.
    pub fn insert(&mut self, key: u32, value: InibinValue);

    /// Remove a value by hash key from all buckets.
    /// Returns the removed value if found.
    pub fn remove(&mut self, key: u32) -> Option<InibinValue>;

    /// Check if a key exists in any bucket.
    pub fn contains_key(&self, key: u32) -> bool;

    /// Iterate over all key-value pairs across all buckets.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &InibinValue)>;

    /// Get a reference to a specific set by flag type.
    pub fn set(&self, flags: InibinFlags) -> Option<&InibinSet>;

    /// Get a mutable reference to a specific set by flag type.
    pub fn set_mut(&mut self, flags: InibinFlags) -> Option<&mut InibinSet>;
}
```

### InibinSet

```rust
/// A typed bucket of key-value pairs.
pub struct InibinSet { /* properties map */ }

impl InibinSet {
    /// Get value by hash key within this set.
    pub fn get(&self, key: u32) -> Option<&InibinValue>;

    /// Insert a key-value pair into this set.
    pub fn insert(&mut self, key: u32, value: InibinValue);

    /// Remove a key-value pair from this set.
    pub fn remove(&mut self, key: u32) -> Option<InibinValue>;

    /// Number of entries in this set.
    pub fn len(&self) -> usize;

    /// Whether this set is empty.
    pub fn is_empty(&self) -> bool;

    /// The flag type of this set.
    pub fn set_type(&self) -> InibinFlags;

    /// Iterate over key-value pairs in this set.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &InibinValue)>;
}
```

### InibinFlags

```rust
bitflags! {
    /// Bitfield representing inibin value set types.
    pub struct InibinFlags: u16 {
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

### InibinValue

```rust
/// Typed value stored in an inibin set.
pub enum InibinValue {
    Int32(i32),
    F32(f32),
    U8(f32),
    Int16(i16),
    Int8(u8),
    Bool(bool),
    Vec3U8(Vec3),
    Vec3F32(Vec3),
    Vec2U8(Vec2),
    Vec2F32(Vec2),
    Vec4U8(Vec4),
    Vec4F32(Vec4),
    String(String),
    Int64(i64),
}

impl InibinValue {
    /// Returns the InibinFlags variant this value belongs to.
    pub fn flags(&self) -> InibinFlags;
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
