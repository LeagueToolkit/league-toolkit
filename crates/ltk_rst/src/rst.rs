use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};

use crate::error::RstError;
use crate::hash::RstHash;
use crate::version::{HashBits, RstFormat, RstHashAlgo, RstVersion};

/// Magic bytes at the start of every RST file: `"RST"`.
pub const MAGIC: &[u8; 3] = b"RST";

/// A resolved entry value, borrowed from the table's data blob.
///
/// Internal only: the public API speaks in `&str`; this enum is the single
/// resolution point shared by the accessors and the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Resolved<'a> {
    /// A normal NUL-terminated, valid-UTF-8 string.
    Str(&'a str),
    /// An encrypted (`trenc`) entry: its raw, length-prefixed payload bytes.
    Encrypted(&'a [u8]),
    /// A NUL-terminated entry whose bytes were not valid UTF-8.
    Invalid(&'a [u8]),
}

/// Backing store for a single entry's value.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Slot {
    /// Byte offset into [`Stringtable::blob`] — entries that were read from a file.
    Blob(u32),
    /// An owned string inserted or edited after construction.
    Owned(Box<str>),
}

/// A League of Legends string table (`RST`, "Riot String Table").
///
/// Entries map a (masked) [`RstHash`] to a UTF-8 string.  Strings are kept in a
/// single owned data blob and addressed by offset, so reading the whole file is
/// one allocation and lookups return borrowed [`&str`](str) — no per-entry
/// allocation.  Newly inserted or edited entries are stored inline and don't
/// touch the original blob.
///
/// # Reading
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::Stringtable;
///
/// let mut file = File::open("en_us/bootstrap.stringtable")?;
/// let table = Stringtable::from_reader(&mut file)?;
///
/// if let Some(text) = table.get_key("game_client_quit") {
///     println!("{text}");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Editing and writing
///
/// ```no_run
/// use std::fs::File;
/// use ltk_rst::Stringtable;
///
/// let mut file = File::open("en_us/bootstrap.stringtable")?;
/// let mut table = Stringtable::from_reader(&mut file)?;
///
/// table.insert_str("game_client_play", "Play");      // add
/// table.insert_str("game_client_quit", "Exit");      // edit existing
///
/// let mut out = File::create("out.stringtable")?;
/// table.to_writer(&mut out)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct Stringtable {
    format: RstFormat,
    /// Optional V2 font-config block, preserved verbatim for round-tripping
    /// (not necessarily valid UTF-8).
    font_config: Option<Vec<u8>>,
    /// Raw string-data section; [`Slot::Blob`] offsets index into this.
    blob: Vec<u8>,
    /// Hash → value slot.
    entries: HashMap<RstHash, Slot>,
}

impl Stringtable {
    /// Creates an empty table targeting the latest format ([`RstFormat::LATEST`]).
    pub fn new() -> Self {
        Self::with_format(RstFormat::LATEST)
    }

    /// Creates an empty table that will hash keys and write using `format`.
    pub fn with_format(format: RstFormat) -> Self {
        Self {
            format,
            font_config: None,
            blob: Vec::new(),
            entries: HashMap::new(),
        }
    }

    /// The format this table was read as / will be written as.
    pub fn format(&self) -> RstFormat {
        self.format
    }

    /// Overrides the algorithm used to hash key strings in
    /// [`get_key`](Self::get_key) / [`insert_str`](Self::insert_str).
    ///
    /// The algorithm can't be learned from the file; reading defaults it to
    /// modern XXH3 (xxHash64 only for the unambiguous 40-bit V2/V3 case). For key
    /// lookups on a pre-patch-14.15 39-bit file — the one genuinely ambiguous
    /// case — switch to [`Xxh64`](RstHashAlgo::Xxh64) first:
    ///
    /// ```no_run
    /// use ltk_rst::{Stringtable, RstHashAlgo};
    ///
    /// let mut file = std::fs::File::open("old.stringtable")?;
    /// let mut table = Stringtable::from_reader(&mut file)?;
    /// table.set_hash_algo(RstHashAlgo::Xxh64);
    /// let text = table.get_key("game_client_quit");
    /// # let _ = text;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_hash_algo(&mut self, algo: RstHashAlgo) {
        self.format.hash_algo = algo;
    }

    /// Retargets the table to `format`, re-masking entry hashes to its bit-width.
    ///
    /// Narrowing is lossless — a native narrow table stores exactly the masked
    /// hashes.  Entries that collide once narrowed collapse into one; the value
    /// of the numerically largest original hash wins.
    ///
    /// Fails with [`RstError::IncompatibleAlgo`] if `format` hashes with a
    /// different algorithm (stored hashes can't be re-hashed without their
    /// original keys) and with [`RstError::CannotWiden`] if it is wider than the
    /// current format (the masked-off high bits are gone).
    pub fn set_format(&mut self, format: RstFormat) -> Result<(), RstError> {
        if format.hash_algo != self.format.hash_algo {
            return Err(RstError::IncompatibleAlgo {
                from: self.format.hash_algo,
                to: format.hash_algo,
            });
        }
        if format.hash_bits > self.format.hash_bits {
            return Err(RstError::CannotWiden {
                from: self.format.hash_bits,
                to: format.hash_bits,
            });
        }
        if format.hash_bits < self.format.hash_bits {
            let mask = format.hash_mask();
            let mut old: Vec<(RstHash, Slot)> =
                std::mem::take(&mut self.entries).into_iter().collect();
            old.sort_unstable_by_key(|(hash, _)| hash.0);
            self.entries = HashMap::with_capacity(old.len());
            for (hash, slot) in old {
                self.entries.insert(RstHash(hash.0 & mask), slot);
            }
        }
        self.format = format;
        Ok(())
    }

    /// Retargets the table to the current live-game format ([`RstFormat::LATEST`]).
    ///
    /// Shorthand for [`set_format(RstFormat::LATEST)`](Self::set_format).
    pub fn to_latest(&mut self) -> Result<(), RstError> {
        self.set_format(RstFormat::LATEST)
    }

    /// The optional V2 font-config block as raw bytes, if present.
    pub fn font_config(&self) -> Option<&[u8]> {
        self.font_config.as_deref()
    }

    /// The optional V2 font-config block as a string.
    pub fn font_config_str(&self) -> Option<Cow<'_, str>> {
        self.font_config.as_deref().map(String::from_utf8_lossy)
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the table has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Masks a caller-supplied hash to this table's bit-width, matching how
    /// hashes are stored — so a full 64-bit hash never silently misses.
    fn masked(&self, hash: impl Into<RstHash>) -> RstHash {
        RstHash(hash.into().0 & self.format.hash_mask())
    }

    /// Whether an entry with `hash` exists.
    pub fn contains(&self, hash: impl Into<RstHash>) -> bool {
        self.entries.contains_key(&self.masked(hash))
    }

    /// Returns the string for `hash`, if it is a valid-UTF-8 string entry.
    ///
    /// `hash` is masked to this table's bit-width first, so passing a full
    /// (untruncated) 64-bit hash is fine.  Returns `None` for missing entries,
    /// encrypted entries, or entries whose bytes weren't valid UTF-8 — use
    /// [`get_lossy`](Self::get_lossy) to recover invalid bytes or
    /// [`get_encrypted`](Self::get_encrypted) for locked ones.
    pub fn get(&self, hash: impl Into<RstHash>) -> Option<&str> {
        match self.raw_value(self.entries.get(&self.masked(hash))?) {
            Resolved::Str(s) => Some(s),
            _ => None,
        }
    }

    /// Looks up an entry by key string, hashing it with this table's format.
    pub fn get_key(&self, key: impl AsRef<str>) -> Option<&str> {
        self.get(self.format.hash_of(key))
    }

    /// Whether the entry for `hash` is an encrypted (`trenc`) entry.
    ///
    /// Encrypted entries are Riot's anti-datamining wrapper around unreleased
    /// strings; [`get`](Self::get) and [`iter`](Self::iter) skip them.  Reach the
    /// raw bytes with [`get_encrypted`](Self::get_encrypted).
    pub fn is_encrypted(&self, hash: impl Into<RstHash>) -> bool {
        matches!(
            self.entries
                .get(&self.masked(hash))
                .map(|slot| self.raw_value(slot)),
            Some(Resolved::Encrypted(_))
        )
    }

    /// Returns the raw payload bytes of an encrypted (`trenc`) entry.
    ///
    /// Returns `None` for missing entries and for ordinary string entries.
    pub fn get_encrypted(&self, hash: impl Into<RstHash>) -> Option<&[u8]> {
        match self.raw_value(self.entries.get(&self.masked(hash))?) {
            Resolved::Encrypted(b) => Some(b),
            _ => None,
        }
    }

    /// Returns the value for `hash` as a (possibly lossy) string.
    ///
    /// Borrows for valid UTF-8, allocates with replacement characters for
    /// invalid bytes, and returns `None` for encrypted entries.
    pub fn get_lossy(&self, hash: impl Into<RstHash>) -> Option<Cow<'_, str>> {
        match self.raw_value(self.entries.get(&self.masked(hash))?) {
            Resolved::Str(s) => Some(Cow::Borrowed(s)),
            Resolved::Invalid(b) => Some(String::from_utf8_lossy(b)),
            Resolved::Encrypted(_) => None,
        }
    }

    /// Iterates over valid-UTF-8 string entries as `(hash, &str)`.
    ///
    /// Encrypted and non-UTF-8 entries are skipped; use [`keys`](Self::keys) to
    /// enumerate every hash regardless of kind.
    pub fn iter(&self) -> impl Iterator<Item = (RstHash, &str)> {
        self.entries
            .iter()
            .filter_map(|(h, slot)| match self.raw_value(slot) {
                Resolved::Str(s) => Some((*h, s)),
                _ => None,
            })
    }

    /// Iterates over the hash of every entry, including encrypted and invalid
    /// ones.
    pub fn keys(&self) -> impl Iterator<Item = RstHash> + '_ {
        self.entries.keys().copied()
    }

    /// Inserts or replaces the entry for a pre-computed `hash`.
    ///
    /// `hash` is masked to this table's bit-width first, so a full (untruncated)
    /// 64-bit hash is stored exactly as a native file would store it.
    pub fn insert(&mut self, hash: impl Into<RstHash>, value: impl Into<String>) {
        let value: String = value.into();
        self.entries
            .insert(self.masked(hash), Slot::Owned(value.into_boxed_str()));
    }

    /// Hashes `key` with this table's format and inserts or replaces the entry.
    pub fn insert_str(&mut self, key: impl AsRef<str>, value: impl Into<String>) {
        let hash = self.format.hash_of(key);
        self.insert(hash, value);
    }

    /// Removes the entry for `hash`, returning whether it existed.
    pub fn remove(&mut self, hash: impl Into<RstHash>) -> bool {
        let hash = self.masked(hash);
        self.entries.remove(&hash).is_some()
    }

    /// Resolves a slot to its borrowed value.
    fn raw_value<'a>(&'a self, slot: &'a Slot) -> Resolved<'a> {
        match slot {
            Slot::Owned(s) => Resolved::Str(s),
            Slot::Blob(off) => match self.blob.get(*off as usize..) {
                // Unreachable: `Slot::Blob` is only created in `read_with`,
                // which rejects offsets past the blob — kept panic-free
                // defensively rather than asserting.
                None => Resolved::Str(""),
                Some(data) if data.first() == Some(&0xFF) && data.len() >= 3 => {
                    let size = u16::from_le_bytes([data[1], data[2]]) as usize;
                    let end = (3 + size).min(data.len());
                    Resolved::Encrypted(&data[3..end])
                }
                Some(data) => {
                    let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
                    let bytes = &data[..end];
                    match std::str::from_utf8(bytes) {
                        Ok(s) => Resolved::Str(s),
                        Err(_) => Resolved::Invalid(bytes),
                    }
                }
            },
        }
    }

    /// Parses a table from RST data, assuming the latest format parameters.
    ///
    /// Only [`Read`] is required — the whole data section is buffered and
    /// addressed in memory, so no seeking is needed.  V2/V3 use their fixed
    /// 40-bit split; V4/V5 are assumed to be the current 38-bit / XXH3 layout.
    /// For an older V4/V5 file, use [`reader()`](Self::reader) to detect or pin
    /// the split.
    pub fn from_reader(reader: &mut impl Read) -> Result<Self, RstError> {
        Self::reader().read(reader)
    }

    /// Begins a configurable read.
    ///
    /// Defaults to the latest parameters; opt into hash-bit
    /// [detection](RstReader::detect_hash_bits) for older files, or pin the
    /// [bits](RstReader::hash_bits) / [algorithm](RstReader::hash_algo).
    pub fn reader() -> RstReader {
        RstReader::new()
    }

    fn read_with(reader: &mut impl Read, opts: &RstReader) -> Result<Self, RstError> {
        let mut magic = [0u8; 3];
        reader.read_exact(&mut magic)?;
        if &magic != MAGIC {
            return Err(RstError::InvalidMagic { actual: magic });
        }

        let version = RstVersion::try_from(reader.read_u8()?)?;

        let mut font_config = None;
        if version.has_font_config() && reader.read_u8()? != 0 {
            let len = reader.read_u32::<LE>()? as usize;
            let mut buf = vec![0u8; len];
            reader.read_exact(&mut buf)?;

            font_config = Some(buf);
        }

        let count = reader.read_u32::<LE>()? as usize;
        let mut raw = Vec::with_capacity(count);
        for _ in 0..count {
            raw.push(reader.read_u64::<LE>()?);
        }

        // the flag is only a hint; encrypted entries self-describe via their 0xFF marker
        if version.has_encryption_flag() {
            reader.read_u8()?;
        }

        let mut blob = Vec::new();
        reader.read_to_end(&mut blob)?;

        let hash_bits = match opts.hash_bits {
            HashBitsSource::Fixed(bits) => bits,
            HashBitsSource::Latest => version
                .fixed_hash_bits()
                .unwrap_or(RstFormat::LATEST.hash_bits),
            HashBitsSource::Detect => {
                detect_hash_bits(version, &raw, &blob).ok_or(RstError::IndeterminateHashBits {
                    version: version.into(),
                })?
            }
        };
        let hash_algo = opts.hash_algo.unwrap_or_else(|| default_algo(hash_bits));
        let format = RstFormat {
            version,
            hash_bits,
            hash_algo,
        };

        let mut entries = HashMap::with_capacity(count);
        for entry_bytes in raw {
            let (hash, offset) = format.unpack(entry_bytes);
            if offset > blob.len() as u64 {
                return Err(RstError::InvalidOffset { offset });
            }

            entries.insert(hash, Slot::Blob(offset as u32));
        }

        Ok(Self {
            format,
            font_config,
            blob,
            entries,
        })
    }

    /// Serialises this table to `writer` in [`self.format()`](Self::format).
    ///
    /// String data is rebuilt and de-duplicated, so dead bytes left by edits are
    /// dropped. Entries are written sorted by hash, so output is deterministic
    /// and reproducible.
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<(), RstError> {
        let format = self.format;

        writer.write_all(MAGIC)?;
        writer.write_u8(format.version.into())?;

        if format.version.has_font_config() {
            match &self.font_config {
                Some(cfg) => {
                    writer.write_u8(1)?;
                    writer.write_u32::<LE>(cfg.len() as u32)?;
                    writer.write_all(cfg)?;
                }
                None => writer.write_u8(0)?,
            }
        }

        let offset_limit = 1u64 << (64 - format.hash_bits.get());

        let mut blob: Vec<u8> = Vec::new();
        let mut dedup: HashMap<Vec<u8>, u64> = HashMap::with_capacity(self.entries.len());
        let mut packed: Vec<u64> = Vec::with_capacity(self.entries.len());
        let mut any_encrypted = false;

        let mask = format.hash_mask();
        let mut sorted: Vec<(&RstHash, &Slot)> = self.entries.iter().collect();
        sorted.sort_unstable_by_key(|(hash, _)| (hash.0 & mask, hash.0));

        for (hash, slot) in sorted {
            let encoded = match self.raw_value(slot) {
                Resolved::Str(s) => terminated(s.as_bytes()),
                Resolved::Invalid(b) => terminated(b),
                Resolved::Encrypted(b) => {
                    any_encrypted = true;
                    let mut v = Vec::with_capacity(3 + b.len());
                    v.push(0xFF);
                    v.extend_from_slice(&(b.len() as u16).to_le_bytes());
                    v.extend_from_slice(b);
                    v
                }
            };

            let offset = match dedup.get(&encoded) {
                Some(&off) => off,
                None => {
                    let off = blob.len() as u64;
                    blob.extend_from_slice(&encoded);
                    dedup.insert(encoded, off);
                    off
                }
            };

            if offset >= offset_limit {
                return Err(RstError::OffsetOverflow {
                    offset,
                    hash_bits: format.hash_bits,
                });
            }

            packed.push(format.pack(*hash, offset));
        }

        writer.write_u32::<LE>(self.entries.len() as u32)?;
        for entry in &packed {
            writer.write_u64::<LE>(*entry)?;
        }
        if format.version.has_encryption_flag() {
            writer.write_u8(any_encrypted as u8)?;
        }
        writer.write_all(&blob)?;

        Ok(())
    }
}

impl Default for Stringtable {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for Stringtable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Stringtable")
            .field("format", &self.format)
            .field("len", &self.entries.len())
            .field("font_config", &self.font_config.as_ref().map(|c| c.len()))
            .field("blob_len", &self.blob.len())
            .finish_non_exhaustive()
    }
}

/// Semantic equality: format, font config, and resolved entry values.  Where a
/// value lives (original blob vs. inline edit) doesn't matter, so a table
/// compares equal to its written-and-reloaded self.
impl PartialEq for Stringtable {
    fn eq(&self, other: &Self) -> bool {
        self.format == other.format
            && self.font_config == other.font_config
            && self.entries.len() == other.entries.len()
            && self.entries.iter().all(|(hash, slot)| {
                other
                    .entries
                    .get(hash)
                    .is_some_and(|other_slot| self.raw_value(slot) == other.raw_value(other_slot))
            })
    }
}

impl Eq for Stringtable {}

/// How a [`RstReader`] resolves the V4/V5 hash/offset split.
#[derive(Debug, Clone, Copy)]
enum HashBitsSource {
    /// Assume the latest split ([`RstFormat::LATEST`], 38-bit); V2/V3 stay 40.
    Latest,
    /// Auto-detect from the data section (walking the entry layout).
    Detect,
    /// Use exactly this width.
    Fixed(HashBits),
}

/// A configurable reader for [`Stringtable`], created via
/// [`Stringtable::reader`].
///
/// The structural version is always read from the file. `hash_bits` and
/// `hash_algo` aren't derivable from the file, so this controls how they're
/// resolved: by default it assumes the latest parameters (V4/V5 → 38-bit / XXH3,
/// V2/V3 → 40-bit / xxHash64). Opt into [`detect_hash_bits`](Self::detect_hash_bits)
/// for an older V4/V5 file, or pin either dimension explicitly.
///
/// ```no_run
/// use ltk_rst::{Stringtable, RstHashAlgo};
///
/// let mut file = std::fs::File::open("bootstrap.stringtable")?;
/// let table = Stringtable::reader()
///     .detect_hash_bits()          // auto-detect 39 vs 38 for an older file
///     .read(&mut file)?;
/// # let _ = table;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct RstReader {
    hash_bits: HashBitsSource,
    hash_algo: Option<RstHashAlgo>,
}

impl RstReader {
    fn new() -> Self {
        Self {
            hash_bits: HashBitsSource::Latest,
            hash_algo: None,
        }
    }

    /// Auto-detect the V4/V5 hash/offset split from the data instead of assuming
    /// the latest. Fails with [`RstError::IndeterminateHashBits`] if the split
    /// can't be determined — no width fits the data, or both fit but decode it
    /// differently.
    pub fn detect_hash_bits(mut self) -> Self {
        self.hash_bits = HashBitsSource::Detect;
        self
    }

    /// Pin the hash/offset split explicitly ([`B40`](HashBits::B40) /
    /// [`B39`](HashBits::B39) / [`B38`](HashBits::B38)), skipping detection.
    pub fn hash_bits(mut self, bits: HashBits) -> Self {
        self.hash_bits = HashBitsSource::Fixed(bits);
        self
    }

    /// Pin the key-hashing algorithm. Defaults to the width-appropriate choice
    /// (xxHash64 for 40-bit, otherwise XXH3); only affects key lookups.
    pub fn hash_algo(mut self, algo: RstHashAlgo) -> Self {
        self.hash_algo = Some(algo);
        self
    }

    /// Read a [`Stringtable`] from `reader` using this configuration.
    pub fn read(&self, reader: &mut impl Read) -> Result<Stringtable, RstError> {
        Stringtable::read_with(reader, self)
    }
}

/// Appends a NUL terminator to `bytes`.
fn terminated(bytes: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(bytes.len() + 1);
    v.extend_from_slice(bytes);
    v.push(0);
    v
}

/// The hash algorithm to assume for a freshly-read file, by bit-width.
///
/// Reading never uses this — it only matters for later key-string lookups, and
/// can be changed with [`Stringtable::set_hash_algo`] or pinned up front via
/// [`RstReader::hash_algo`].
///
/// 40-bit (V2/V3) is always xxHash64 and 38-bit is always XXH3.  39-bit is
/// genuinely ambiguous (xxHash64 before patch 14.15, XXH3 from 14.15 to 15.1);
/// we guess the modern XXH3, so pre-14.15 files need an override.
fn default_algo(hash_bits: HashBits) -> RstHashAlgo {
    match hash_bits {
        HashBits::B40 => RstHashAlgo::Xxh64,
        _ => RstHashAlgo::Xxh3,
    }
}

/// Determines the hash/offset split for a file.
///
/// V2/V3 are fixed at 40 bits.  For V4/V5, each candidate shift (39 vs 38) is
/// tested against the real entry layout from [`entry_starts`]: a shift is valid
/// only if every entry's offset lands on an entry start — a wrong shift sends
/// offsets out of bounds or into the middle of an entry.  When both widths fit,
/// the tie is harmless only if every entry decodes to the same `(hash, offset)`
/// under either width — equivalent to every raw value fitting in 38 bits — and
/// then the newer 38 is used; a tie where the decodes differ is genuinely
/// ambiguous and detection fails rather than guessing.
fn detect_hash_bits(version: RstVersion, raw: &[u64], blob: &[u8]) -> Option<HashBits> {
    if let Some(bits) = version.fixed_hash_bits() {
        return Some(bits);
    }

    let starts = entry_starts(blob);
    let fits = |bits: HashBits| {
        raw.iter()
            .all(|&v| starts.contains(&((v >> bits.get()) as usize)))
    };
    match (fits(HashBits::B39), fits(HashBits::B38)) {
        (true, false) => Some(HashBits::B39),
        (false, true) => Some(HashBits::B38),
        (true, true) if raw.iter().all(|&v| v >> 38 == 0) => Some(HashBits::B38),
        _ => None,
    }
}

/// Reconstructs the set of valid entry-start offsets by walking `blob`.
///
/// Entries are laid out contiguously, each either a NUL-terminated string or a
/// `0xFF`-marked encrypted blob (`0xFF` + little-endian `u16` length + payload).
/// Walking is required because the offset right after an encrypted blob is a
/// valid start without being preceded by a NUL.
fn entry_starts(blob: &[u8]) -> HashSet<usize> {
    let mut starts = HashSet::new();
    let mut pos = 0;
    while pos < blob.len() {
        starts.insert(pos);
        if blob[pos] == 0xFF && pos + 3 <= blob.len() {
            let size = u16::from_le_bytes([blob[pos + 1], blob[pos + 2]]) as usize;
            pos += 3 + size;
        } else {
            match blob[pos..].iter().position(|&b| b == 0) {
                Some(n) => pos += n + 1,
                None => break, // unterminated trailing bytes — no further starts
            }
        }
    }
    starts.insert(blob.len()); // offset == len is a valid empty entry
    starts
}
