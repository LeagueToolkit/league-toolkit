use num_enum::{IntoPrimitive, TryFromPrimitive};

/// A typed value stored in a troybin entry.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Value {
    /// Integer (from i32, i16, u8, bool blocks).
    Int(i32),
    /// Floating-point scalar.
    Float(f64),
    /// String value.
    String(String),
    /// Multi-component vector (2, 3, or 4 elements).
    Vec(Vec<f64>),
}

impl Value {
    /// Format the value for INI text output.
    pub fn to_ini_string(&self) -> String {
        match self {
            Value::Int(v) => v.to_string(),
            Value::Float(v) => {
                if v.is_nan() {
                    "NaN".to_string()
                } else {
                    format!("{}", v)
                }
            }
            Value::String(s) => {
                if s.parse::<f64>().is_ok() {
                    s.clone()
                } else {
                    format!("\"{}\"", s)
                }
            }
            Value::Vec(vals) => vals
                .iter()
                .map(|v| format!("{:.1}", v))
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

/// Which binary storage type was used for this entry.
///
/// Preserving this enables lossless round-trip: binary -> struct -> binary.
/// The discriminant values correspond to flag bit indices in the v2 binary format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum StorageType {
    /// Bit 0: `i32 x 1, mul=1`
    Int32 = 0,
    /// Bit 1: `f32 x 1, mul=1`
    Float32 = 1,
    /// Bit 2: `u8 x 1, mul=0.1`
    U8Scaled = 2,
    /// Bit 3: `i16 x 1, mul=1`
    Int16 = 3,
    /// Bit 4: `u8 x 1, mul=1`
    U8 = 4,
    /// Bit 5: packed booleans
    Bool = 5,
    /// Bit 6: `u8 x 3, mul=0.1`
    U8x3Scaled = 6,
    /// Bit 7: `f32 x 3, mul=1`
    Float32x3 = 7,
    /// Bit 8: `u8 x 2, mul=0.1`
    U8x2Scaled = 8,
    /// Bit 9: `f32 x 2, mul=1`
    Float32x2 = 9,
    /// Bit 10: `u8 x 4, mul=0.1` (colors)
    U8x4Scaled = 10,
    /// Bit 11: `f32 x 4, mul=1`
    Float32x4 = 11,
    /// Bit 12: null-terminated strings
    StringBlock = 12,
    /// Bit 13: `i32 x 1, mul=1` (long in Leischii's code)
    Int32Long = 13,
    /// Old format (v1) — all values stored as strings in a data block.
    /// Uses discriminant 255 since it's not a v2 flag bit.
    OldFormat = 255,
}

impl StorageType {
    /// Number of scalar components per entry for this type.
    pub fn component_count(self) -> usize {
        match self {
            StorageType::U8x3Scaled | StorageType::Float32x3 => 3,
            StorageType::U8x2Scaled | StorageType::Float32x2 => 2,
            StorageType::U8x4Scaled | StorageType::Float32x4 => 4,
            _ => 1,
        }
    }

    /// Multiplier applied when reading raw values.
    pub fn multiplier(self) -> f64 {
        match self {
            StorageType::U8Scaled
            | StorageType::U8x3Scaled
            | StorageType::U8x2Scaled
            | StorageType::U8x4Scaled => 0.1,
            _ => 1.0,
        }
    }
}

/// A single raw entry: hash + value + storage type.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RawEntry {
    /// The ihash of the field path.
    pub hash: u32,
    /// Decoded value.
    pub value: Value,
    /// How this was stored in the binary (preserved for round-trip).
    pub storage: StorageType,
}

/// A resolved property within a section.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Property {
    /// Human-readable field name, or raw hash string if unresolved.
    pub name: String,
    /// The value.
    pub value: Value,
    /// Original hash (for binary round-trip).
    pub hash: u32,
    /// Storage type (for binary round-trip).
    pub storage: StorageType,
}

/// A group/section of properties (e.g. "System", emitter name, "UNKNOWN_HASHES").
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Section {
    /// Section name (e.g. "System", "BuffBone_Glb_Center_Loc").
    pub name: String,
    /// Properties in this section.
    pub properties: Vec<Property>,
}

/// A fully parsed and resolved troybin document.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Troybin {
    /// Format version (1 = old, 2 = new).
    pub version: u8,
    /// Resolved sections with named properties.
    pub sections: Vec<Section>,
    /// Entries whose hash could not be resolved.
    pub unknown_entries: Vec<RawEntry>,
    /// All raw entries (preserved for binary round-trip).
    pub raw_entries: Vec<RawEntry>,
}
