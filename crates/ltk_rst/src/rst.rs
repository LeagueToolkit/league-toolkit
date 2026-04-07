use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use ltk_io_ext::ReaderExt as _;

use crate::error::RstError;
use crate::hash::{compute_hash, pack_entry, unpack_entry};
use crate::version::RstVersion;

/// Magic bytes at the start of every RST file: `"RST"`.
pub const MAGIC: &[u8; 3] = b"RST";

/// A parsed string table.
///
/// String tables are League of Legends localisation tables that map
/// XXHash64-based keys to UTF-8 strings.  The hash table entries pack both
/// the string hash and the offset of its null-terminated UTF-8 data into a
/// single `u64`.
///
/// # Reading
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::Stringtable;
///
/// let mut file = File::open("fontconfig_en_us.stringtable")?;
/// let table = Stringtable::from_rst_reader(&mut file)?;
///
/// if let Some(text) = table.get(0x1234_5678_9abc_def0) {
///     println!("{text}");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Writing
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::Stringtable;
///
/// let mut table = Stringtable::new();
/// table.insert_str("game_client_quit", "Quit");
///
/// let mut out = File::create("out.stringtable")?;
/// table.to_rst_writer(&mut out)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stringtable {
    /// Hash → string mapping.
    pub entries: HashMap<u64, String>,
}

impl Stringtable {
    /// Creates an empty [`Stringtable`].
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Returns the number of entries in the table.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the table contains no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns an iterator over the entries in the table.
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &String)> {
        self.entries.iter()
    }

    /// Returns the string associated with `hash`, if any.
    pub fn get(&self, hash: u64) -> Option<&str> {
        self.entries.get(&hash).map(|s| s.as_str())
    }

    /// Inserts an entry by pre-computed hash.
    ///
    /// The hash must already be masked to the bit-width of the desired
    /// [`RstHashType`] — use [`compute_hash`] to produce it.
    pub fn insert(&mut self, hash: u64, value: impl Into<String>) {
        self.entries.insert(hash, value.into());
    }

    /// Hashes `key` using the latest version's hash type and inserts the entry.
    pub fn insert_str(&mut self, key: &str, value: impl Into<String>) {
        let hash = compute_hash(key, RstVersion::V5.hash_type());
        self.insert(hash, value);
    }

    /// Parses a [`Stringtable`] from any [`Read`] + [`Seek`] source containing
    /// RST data.
    ///
    /// Seeking is required because string data offsets stored in the hash
    /// table are relative to the start of the string section, which is only
    /// known after the entire hash table has been read.
    pub fn from_rst_reader(reader: &mut (impl Read + Seek)) -> Result<Self, RstError> {
        let mut magic = [0u8; 3];
        reader.read_exact(&mut magic)?;
        if magic != *MAGIC {
            return Err(RstError::InvalidMagic { actual: magic });
        }

        let version = RstVersion::try_from_u8(reader.read_u8()?)?;
        let hash_type = version.hash_type();

        // V2 has an optional font-config string (read and discard).
        if version == RstVersion::V2 {
            let has_config = reader.read_u8()? != 0;
            if has_config {
                let len = reader.read_i32::<LE>()?;
                reader.seek(SeekFrom::Current(len as i64))?;
            }
        }

        let count = reader.read_i32::<LE>()? as usize;
        let mut pairs: Vec<(u64, u64)> = Vec::with_capacity(count);
        for _ in 0..count {
            let raw = reader.read_u64::<LE>()?;
            pairs.push(unpack_entry(raw, hash_type));
        }

        // V2–V4 have a mode byte (read and discard).
        if version.has_mode_byte() {
            let _ = reader.read_u8()?;
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

        Ok(Self { entries })
    }

    /// Serialises this [`Stringtable`] to any [`Write`] sink as RST V5.
    pub fn to_rst_writer(&self, writer: &mut impl Write) -> Result<(), RstError> {
        use ltk_io_ext::WriterExt as _;
        let hash_type = RstVersion::V5.hash_type();

        writer.write_all(MAGIC)?;
        writer.write_u8(RstVersion::V5.to_u8())?;

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
                data.write_terminated_string(text)?;
                text_to_offset.insert(text.as_str(), off);
                off
            };

            let packed = pack_entry(*hash, offset, hash_type);
            packed_entries.push(packed);
        }

        for packed in &packed_entries {
            writer.write_u64::<LE>(*packed)?;
        }

        writer.write_all(&data)?;

        Ok(())
    }
}

impl Default for Stringtable {
    fn default() -> Self {
        Self::new()
    }
}
