use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use ltk_io_ext::ReaderExt as _;

use crate::error::RstError;
use crate::hash::{compute_hash, pack_entry, unpack_entry};
use crate::version::{RstMode, RstVersion};

/// Magic bytes at the start of every RST file: `"RST"`.
pub const MAGIC: &[u8; 3] = b"RST";

/// A parsed RST (Riot String Table) file.
///
/// RST files are League of Legends localisation tables that map XXHash64-based
/// keys to UTF-8 strings.  The hash table entries pack both the string hash and
/// the offset of its null-terminated UTF-8 data into a single `u64`.
///
/// # Reading
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::RstFile;
///
/// let mut file = File::open("fontconfig_en_us.stringtable")?;
/// let rst = RstFile::from_reader(&mut file)?;
///
/// if let Some(text) = rst.get(0x1234_5678_9abc_def0) {
///     println!("{text}");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Writing
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::{RstFile, RstVersion};
///
/// let mut rst = RstFile::new(RstVersion::V5);
/// rst.insert_str("game_client_quit", "Quit");
///
/// let mut out = File::create("out.stringtable")?;
/// rst.to_writer(&mut out)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RstFile {
    /// File version (encodes config and mode alongside version-specific data).
    pub version: RstVersion,

    /// Hash → string mapping.
    pub entries: HashMap<u64, String>,
}

impl RstFile {
    /// Creates an empty [`RstFile`] for the given version.
    pub fn new(version: RstVersion) -> Self {
        Self {
            version,
            entries: HashMap::new(),
        }
    }

    /// Returns the string associated with `hash`, if any.
    pub fn get(&self, hash: u64) -> Option<&str> {
        self.entries.get(&hash).map(|s| s.as_str())
    }

    /// Inserts an entry by pre-computed hash.
    ///
    /// The hash must already be masked to the bit-width of the file's
    /// [`RstHashType`] — use [`compute_hash`] to produce it.
    pub fn insert(&mut self, hash: u64, value: impl Into<String>) {
        self.entries.insert(hash, value.into());
    }

    /// Hashes `key` for this file's version and inserts the entry.
    pub fn insert_str(&mut self, key: &str, value: impl Into<String>) {
        let hash = compute_hash(key, self.version.hash_type());
        self.insert(hash, value);
    }

    /// Parses an [`RstFile`] from any [`Read`] + [`Seek`] source.
    ///
    /// Seeking is required because string data offsets stored in the hash
    /// table are relative to the start of the string section, which is only
    /// known after the entire hash table has been read.
    pub fn from_reader(reader: &mut (impl Read + Seek)) -> Result<Self, RstError> {
        let mut magic = [0u8; 3];
        reader.read_exact(&mut magic)?;
        if magic != *MAGIC {
            return Err(RstError::InvalidMagic { actual: magic });
        }

        let version_byte = reader.read_u8()?;
        let mut version = RstVersion::try_from_u8(version_byte)?;
        let hash_type = version.hash_type();

        // Read config for V2
        if let RstVersion::V2 { ref mut config, .. } = version {
            let has_config = reader.read_u8()? != 0;
            if has_config {
                let len = reader.read_i32::<LE>()? as usize;
                let mut buf = vec![0u8; len];
                reader.read_exact(&mut buf)?;
                *config = Some(String::from_utf8_lossy(&buf).into_owned());
            }
        }

        let count = reader.read_i32::<LE>()? as usize;
        let mut pairs: Vec<(u64, u64)> = Vec::with_capacity(count);
        for _ in 0..count {
            let raw = reader.read_u64::<LE>()?;
            pairs.push(unpack_entry(raw, hash_type));
        }

        // Read mode byte for versions that have it
        if version.has_mode_byte() {
            let mode = RstMode::from_u8(reader.read_u8()?);
            match &mut version {
                RstVersion::V2 {
                    mode: ref mut m, ..
                } => *m = mode,
                RstVersion::V3 { mode: ref mut m } => *m = mode,
                RstVersion::V4 { mode: ref mut m } => *m = mode,
                _ => {}
            }
        }

        let data_start = reader.stream_position()?;
        let mut offset_cache: HashMap<u64, String> = HashMap::with_capacity(count);
        let mut entries: HashMap<u64, String> = HashMap::with_capacity(count);

        for (hash, offset) in pairs {
            let text = if let Some(cached) = offset_cache.get(&offset) {
                cached.clone()
            } else {
                reader.seek(SeekFrom::Start(data_start + offset))?;
                let text = reader.read_str_until_nul()?;
                offset_cache.insert(offset, text.clone());
                text
            };
            entries.insert(hash, text);
        }

        Ok(Self { version, entries })
    }

    /// Serialises this [`RstFile`] to any [`Write`] sink.
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<(), RstError> {
        let hash_type = self.version.hash_type();

        // Write magic + version byte
        writer.write_all(MAGIC)?;
        writer.write_u8(self.version.to_u8())?;

        // Write config for V2
        if let RstVersion::V2 { ref config, .. } = self.version {
            match config {
                Some(cfg) if !cfg.is_empty() => {
                    writer.write_u8(1)?;
                    writer.write_i32::<LE>(cfg.len() as i32)?;
                    writer.write_all(cfg.as_bytes())?;
                }
                _ => {
                    writer.write_u8(0)?;
                }
            }
        }

        // Write entry count
        writer.write_i32::<LE>(self.entries.len() as i32)?;

        // Build string data blob with deduplication, and collect packed entries
        let mut data: Vec<u8> = Vec::new();
        let mut text_to_offset: HashMap<&str, u64> = HashMap::with_capacity(self.entries.len());
        let mut packed_entries: Vec<u64> = Vec::with_capacity(self.entries.len());

        for (hash, text) in &self.entries {
            let offset = if let Some(&off) = text_to_offset.get(text.as_str()) {
                off
            } else {
                let off = data.len() as u64;
                data.extend_from_slice(text.as_bytes());
                data.push(0x00);
                text_to_offset.insert(text.as_str(), off);
                off
            };

            let packed = pack_entry(*hash, offset, hash_type);
            packed_entries.push(packed);
        }

        // Write packed hash-table entries
        for packed in &packed_entries {
            writer.write_u64::<LE>(*packed)?;
        }

        // Write mode byte if applicable
        if self.version.has_mode_byte() {
            writer.write_u8(self.version.mode() as u8)?;
        }

        // Write string data
        writer.write_all(&data)?;

        Ok(())
    }
}
