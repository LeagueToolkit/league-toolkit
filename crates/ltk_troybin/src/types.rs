/// A typed value stored in a troybin entry.
#[derive(Debug, Clone, PartialEq)]
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
/// Preserving this enables lossless round-trip: binary → struct → binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    /// Bit 0: `i32 × 1, mul=1`
    Int32,
    /// Bit 1: `f32 × 1, mul=1`
    Float32,
    /// Bit 2: `u8 × 1, mul=0.1`
    U8Scaled,
    /// Bit 3: `i16 × 1, mul=1`
    Int16,
    /// Bit 4: `u8 × 1, mul=1`
    U8,
    /// Bit 5: packed booleans
    Bool,
    /// Bit 6: `u8 × 3, mul=0.1`
    U8x3Scaled,
    /// Bit 7: `f32 × 3, mul=1`
    Float32x3,
    /// Bit 8: `u8 × 2, mul=0.1`
    U8x2Scaled,
    /// Bit 9: `f32 × 2, mul=1`
    Float32x2,
    /// Bit 10: `u8 × 4, mul=0.1` (colors)
    U8x4Scaled,
    /// Bit 11: `f32 × 4, mul=1`
    Float32x4,
    /// Bit 12: null-terminated strings
    StringBlock,
    /// Bit 13: `i32 × 1, mul=1` (long in Leischii's code)
    Int32Long,
    /// Old format (v1) — all values stored as strings in a data block
    OldFormat,
}

impl StorageType {
    /// The flag bit index for this storage type (0–13).
    pub fn bit_index(self) -> u16 {
        match self {
            StorageType::Int32 => 0,
            StorageType::Float32 => 1,
            StorageType::U8Scaled => 2,
            StorageType::Int16 => 3,
            StorageType::U8 => 4,
            StorageType::Bool => 5,
            StorageType::U8x3Scaled => 6,
            StorageType::Float32x3 => 7,
            StorageType::U8x2Scaled => 8,
            StorageType::Float32x2 => 9,
            StorageType::U8x4Scaled => 10,
            StorageType::Float32x4 => 11,
            StorageType::StringBlock => 12,
            StorageType::Int32Long => 13,
            StorageType::OldFormat => 0, // not used for v2 flags
        }
    }

    /// Create from a v2 flag bit index.
    pub fn from_bit(bit: u16) -> Option<Self> {
        match bit {
            0 => Some(StorageType::Int32),
            1 => Some(StorageType::Float32),
            2 => Some(StorageType::U8Scaled),
            3 => Some(StorageType::Int16),
            4 => Some(StorageType::U8),
            5 => Some(StorageType::Bool),
            6 => Some(StorageType::U8x3Scaled),
            7 => Some(StorageType::Float32x3),
            8 => Some(StorageType::U8x2Scaled),
            9 => Some(StorageType::Float32x2),
            10 => Some(StorageType::U8x4Scaled),
            11 => Some(StorageType::Float32x4),
            12 => Some(StorageType::StringBlock),
            13 => Some(StorageType::Int32Long),
            _ => None,
        }
    }

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
pub struct Section {
    /// Section name (e.g. "System", "BuffBone_Glb_Center_Loc").
    pub name: String,
    /// Properties in this section.
    pub properties: Vec<Property>,
}

/// A fully parsed and resolved troybin document.
#[derive(Debug, Clone)]
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
