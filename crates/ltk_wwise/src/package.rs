//! Riot WPK packages (`.wpk`, magic `r3d2`).
//!
//! A package holds WEM media, each under the name `<source ID>.wem`. [`Package`] reads a whole
//! package into memory. [`MountedPackage`] reads the entry records and leaves the media in the
//! source.
//!
//! `docs/design/wwise-audio.md` section 5 specifies the format.

use std::{
    collections::HashSet,
    fmt,
    io::{self, Read, Seek, SeekFrom, Write},
    mem,
};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use ltk_hash::elf;

use crate::{error::to_u32, Error, Result, SourceId};

/// The magic of a package.
const MAGIC: [u8; 4] = *b"r3d2";
/// The one version of the package format.
const VERSION: u32 = 1;
/// The bytes of the magic, the version and the entry count.
const HEADER_LEN: u64 = 12;
/// The bytes of an entry record before its name: the data offset, the data size, the name length.
const RECORD_HEADER_LEN: u64 = 12;
/// The alignment of each entry record and each media.
const ALIGNMENT: u64 = 8;
/// The largest buffer that the reader reserves for one media before it reads the media.
///
/// A media size comes from the file. The buffer grows past this bound as the bytes arrive.
const MAX_RESERVE: u64 = 16 * 1024 * 1024;

/// The key under which the client registers a package entry.
///
/// ```
/// use ltk_wwise::package::PackageKey;
///
/// assert_eq!(PackageKey::from_name("1004362275.wem"), PackageKey(1004362275));
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageKey(pub u64);

/// Writes the decimal value.
impl fmt::Display for PackageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl PackageKey {
    /// Returns the key of the entry name `name`.
    ///
    /// The key is the leading run of ASCII decimal digits of `name`, read as a `u64`. A run that
    /// is empty, reads as 0 or exceeds `u64::MAX` gives the ELF hash of `name` with `A` to `Z`
    /// lowercased.
    pub fn from_name(name: &str) -> Self {
        let leading = name
            .bytes()
            .take_while(u8::is_ascii_digit)
            .try_fold(0u64, |value, digit| {
                value.checked_mul(10)?.checked_add(u64::from(digit - b'0'))
            });
        match leading {
            Some(value) if value != 0 => Self(value),
            _ => Self(elf_lower(name)),
        }
    }

    /// Returns the key of the entry name `<source>.wem`.
    fn from_source(source: SourceId) -> Self {
        match source.0 {
            0 => Self(elf_lower("0.wem")),
            id => Self(u64::from(id)),
        }
    }
}

/// Returns the ELF hash of `name` with `A` to `Z` lowercased.
fn elf_lower(name: &str) -> u64 {
    // The ELF hash keeps 28 bits. A `usize` of 28 bits widens to a `u64` without loss.
    elf::elf(name.to_ascii_lowercase()) as u64
}

/// A package read into memory.
///
/// A package holds at most one entry per [`PackageKey`]. The entries keep the order of the file.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Package {
    entries: Vec<PackageEntry>,
}

impl Package {
    /// Returns a package without entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a package from `reader`.
    ///
    /// The reader reads the header and the entry records, then each media into its own buffer in
    /// ascending offset order. It stops at the end of the last media. A package whose media
    /// overlap, or start inside the bytes of the records, is read to its end into one buffer
    /// first.
    ///
    /// # Errors
    ///
    /// - [`Error::InvalidMagic`] when the file does not start with `r3d2`.
    /// - [`Error::PackageVersion`] when the version is not 1.
    /// - [`Error::EntryOutOfBounds`] when an entry record or a media ends after the end of the
    ///   file.
    /// - [`Error::DuplicateKey`] when two entries have one key.
    /// - [`Error::Io`] when the reader fails, the offset table ends after the end of the file, or
    ///   a name is not valid UTF-16.
    pub fn from_reader(reader: &mut impl Read) -> Result<Self> {
        let mut prefix = Prefix {
            reader,
            bytes: Vec::new(),
        };
        let records = read_records(|offset, len| prefix.fetch(offset, len), None)?;

        let mut order: Vec<usize> = (0..records.len()).collect();
        order.sort_by_key(|&index| records[index].offset);
        let start = prefix.bytes.len() as u64;
        let ascending = order
            .iter()
            .try_fold(start, |end, &index| {
                let record = &records[index];
                let offset = u64::from(record.offset);
                (offset >= end).then_some(offset + u64::from(record.size))
            })
            .is_some();
        let media = if ascending {
            prefix.stream_media(&records, &order)?
        } else {
            prefix.buffer_media(&records)?
        };

        let entries = records
            .into_iter()
            .zip(media)
            .map(|(record, data)| PackageEntry {
                name: record.name,
                key: record.key,
                data,
            })
            .collect();
        Ok(Self { entries })
    }

    /// Writes the package to `writer`.
    ///
    /// The layout is the layout of section 5.1 of the spec, with the entries in order. A package
    /// read from a shipped file writes back byte for byte.
    ///
    /// # Errors
    ///
    /// - [`Error::Io`] when the writer fails, or when an offset or a size of the package exceeds
    ///   `u32::MAX`.
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<()> {
        let layout = Layout::of(&self.entries)?;
        let mut position = 0;

        writer.write_all(&MAGIC)?;
        writer.write_u32::<LE>(VERSION)?;
        writer.write_u32::<LE>(to_u32(self.entries.len(), "package entry count")?)?;
        for slot in &layout.slots {
            writer.write_u32::<LE>(slot.record_offset)?;
        }
        position += layout.table_end;

        for (entry, slot) in self.entries.iter().zip(&layout.slots) {
            position = pad(writer, position, u64::from(slot.record_offset))?;
            writer.write_u32::<LE>(slot.data_offset)?;
            writer.write_u32::<LE>(to_u32(entry.data.len(), "package media size")?)?;
            writer.write_u32::<LE>(slot.name_len)?;
            let mut name = Vec::with_capacity(entry.name.len() * 2);
            for unit in entry.name.encode_utf16() {
                name.extend_from_slice(&unit.to_le_bytes());
            }
            writer.write_all(&name)?;
            position += RECORD_HEADER_LEN + name.len() as u64;
        }

        for (entry, slot) in self.entries.iter().zip(&layout.slots) {
            position = pad(writer, position, u64::from(slot.data_offset))?;
            writer.write_all(&entry.data)?;
            position += entry.data.len() as u64;
        }
        Ok(())
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when the package has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the entries in order.
    pub fn entries(&self) -> &[PackageEntry] {
        &self.entries
    }

    /// Returns the entry with the key `key`.
    pub fn get(&self, key: PackageKey) -> Option<&PackageEntry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// Returns the media of the entry with the key of `source`.
    pub fn wem(&self, source: SourceId) -> Option<&[u8]> {
        self.get(PackageKey::from_source(source))
            .map(PackageEntry::data)
    }

    /// Inserts `entry`. Replaces the entry with the same key in place and returns it.
    ///
    /// An entry of a new key goes after the last entry.
    pub fn insert(&mut self, entry: PackageEntry) -> Option<PackageEntry> {
        match self.entries.iter_mut().find(|slot| slot.key == entry.key) {
            Some(slot) => Some(mem::replace(slot, entry)),
            None => {
                self.entries.push(entry);
                None
            }
        }
    }

    /// Inserts the media `data` as `<source>.wem`. Returns the media it replaces.
    pub fn insert_wem(&mut self, source: SourceId, data: Vec<u8>) -> Option<Vec<u8>> {
        self.insert(PackageEntry::wem(source, data))
            .map(PackageEntry::into_data)
    }

    /// Removes the entry with the key `key` and returns it.
    ///
    /// The other entries keep their order.
    pub fn remove(&mut self, key: PackageKey) -> Option<PackageEntry> {
        let index = self.entries.iter().position(|entry| entry.key == key)?;
        Some(self.entries.remove(index))
    }
}

/// One entry of a package: a name and a media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry {
    name: String,
    key: PackageKey,
    data: Vec<u8>,
}

impl PackageEntry {
    /// Returns the entry `name` with the media `data`.
    pub fn new(name: impl Into<String>, data: Vec<u8>) -> Self {
        let name = name.into();
        let key = PackageKey::from_name(&name);
        Self { name, key, data }
    }

    /// Returns the entry `<source>.wem`.
    pub fn wem(source: SourceId, data: Vec<u8>) -> Self {
        Self {
            name: format!("{source}.wem"),
            key: PackageKey::from_source(source),
            data,
        }
    }

    /// Returns the entry name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the key of the entry name.
    pub fn key(&self) -> PackageKey {
        self.key
    }

    /// Returns the media.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns the media, consuming the entry.
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }
}

/// A package whose entry records are read and whose media stay in the source.
///
/// The `ltk_wad` type `Wad` is the model: `Wad::mount` reads the table of contents and leaves the
/// chunks in the source.
#[derive(Debug)]
pub struct MountedPackage<R> {
    source: R,
    entries: Vec<MountedEntry>,
}

impl<R: Read + Seek> MountedPackage<R> {
    /// Reads the header and the entry records of the package in `source`.
    ///
    /// # Errors
    ///
    /// The errors of [`Package::from_reader`]. The media bounds are checked against the length of
    /// `source`.
    pub fn mount(mut source: R) -> Result<Self> {
        let len = source.seek(SeekFrom::End(0))?;
        let entries = read_records(
            |offset, size| {
                if offset + size > len {
                    return Ok(None);
                }
                source.seek(SeekFrom::Start(offset))?;
                let mut bytes = vec![0; to_usize(size)?];
                source.read_exact(&mut bytes)?;
                Ok(Some(bytes))
            },
            Some(len),
        )?;
        Ok(Self { source, entries })
    }

    /// Returns the entry records in order.
    pub fn entries(&self) -> &[MountedEntry] {
        &self.entries
    }

    /// Returns the entry record with the key `key`.
    pub fn get(&self, key: PackageKey) -> Option<&MountedEntry> {
        self.entries.iter().find(|entry| entry.key == key)
    }

    /// Reads the media of `entry` from the source.
    ///
    /// # Errors
    ///
    /// [`Error::Io`] when the source fails or ends before the end of the media.
    pub fn read(&mut self, entry: &MountedEntry) -> Result<Vec<u8>> {
        read_media(&mut self.source, entry)
    }

    /// Reads every media and returns the package.
    ///
    /// # Errors
    ///
    /// The errors of [`MountedPackage::read`].
    pub fn to_package(&mut self) -> Result<Package> {
        let entries = self
            .entries
            .iter()
            .map(|entry| {
                Ok(PackageEntry {
                    name: entry.name.clone(),
                    key: entry.key,
                    data: read_media(&mut self.source, entry)?,
                })
            })
            .collect::<Result<_>>()?;
        Ok(Package { entries })
    }

    /// Returns the source, consuming the package.
    pub fn into_inner(self) -> R {
        self.source
    }
}

/// The entry record of a mounted package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedEntry {
    name: String,
    key: PackageKey,
    offset: u32,
    size: u32,
}

impl MountedEntry {
    /// Returns the entry name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the key of the entry name.
    pub fn key(&self) -> PackageKey {
        self.key
    }

    /// Returns the absolute offset of the media in the package.
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Returns the size of the media in bytes.
    pub fn size(&self) -> u32 {
        self.size
    }
}

/// Reads the header and the entry records of a package.
///
/// `fetch(offset, len)` returns the `len` bytes of the package at `offset`, or `None` when the
/// package ends before them. `len` is the length of the package when the caller knows it. The
/// media bounds are checked against it.
fn read_records(
    mut fetch: impl FnMut(u64, u64) -> Result<Option<Vec<u8>>>,
    len: Option<u64>,
) -> Result<Vec<MountedEntry>> {
    let header_eof = || io::Error::new(io::ErrorKind::UnexpectedEof, "package header is cut");
    let magic = fetch(0, 4)?.ok_or_else(header_eof)?;
    let Ok(found) = <[u8; 4]>::try_from(magic.as_slice()) else {
        return Err(header_eof().into());
    };
    if found != MAGIC {
        return Err(Error::InvalidMagic {
            expected: MAGIC,
            found,
        });
    }
    let fields = fetch(4, HEADER_LEN - 4)?.ok_or_else(header_eof)?;
    let mut fields = fields.as_slice();
    let version = fields.read_u32::<LE>()?;
    if version != VERSION {
        return Err(Error::PackageVersion(version));
    }
    let count = fields.read_u32::<LE>()?;
    let table = fetch(HEADER_LEN, 4 * u64::from(count))?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "package offset table ends after the end of the package",
        )
    })?;
    let mut table = table.as_slice();

    let mut keys = HashSet::with_capacity(table.len() / 4);
    let mut entries = Vec::with_capacity(table.len() / 4);
    for index in 0..count {
        let record_offset = u64::from(table.read_u32::<LE>()?);
        let record_out_of_bounds = || Error::EntryOutOfBounds {
            name: format!("#{index}"),
        };
        let record = fetch(record_offset, RECORD_HEADER_LEN)?.ok_or_else(record_out_of_bounds)?;
        let mut record = record.as_slice();
        let offset = record.read_u32::<LE>()?;
        let size = record.read_u32::<LE>()?;
        let name_len = record.read_u32::<LE>()?;
        let name = fetch(record_offset + RECORD_HEADER_LEN, 2 * u64::from(name_len))?
            .ok_or_else(record_out_of_bounds)?;
        let units: Vec<u16> = name
            .as_chunks::<2>()
            .0
            .iter()
            .map(|unit| u16::from_le_bytes(*unit))
            .collect();
        let name = String::from_utf16(&units)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        if len.is_some_and(|len| u64::from(offset) + u64::from(size) > len) {
            return Err(Error::EntryOutOfBounds { name });
        }
        let key = PackageKey::from_name(&name);
        if !keys.insert(key) {
            return Err(Error::DuplicateKey(key));
        }
        entries.push(MountedEntry {
            name,
            key,
            offset,
            size,
        });
    }
    Ok(entries)
}

/// The bytes of a package that [`Package::from_reader`] has read, and the rest of the input.
struct Prefix<'a, R> {
    reader: &'a mut R,
    /// The bytes from the start of the package.
    bytes: Vec<u8>,
}

impl<R: Read> Prefix<'_, R> {
    /// Returns the `len` bytes at `offset`, or `None` when the input ends before them.
    ///
    /// The prefix reads the input up to the end of the bytes and keeps every byte that it reads.
    fn fetch(&mut self, offset: u64, len: u64) -> Result<Option<Vec<u8>>> {
        let end = offset + len;
        let read = self.bytes.len() as u64;
        if end > read {
            (&mut *self.reader)
                .take(end - read)
                .read_to_end(&mut self.bytes)?;
        }
        Ok(self
            .bytes
            .get(to_usize(offset)?..to_usize(end)?)
            .map(<[u8]>::to_vec))
    }

    /// Reads the media of `records` from the input into one buffer each, in the order `order`.
    ///
    /// `order` lists the records in ascending offset order. No media starts before the end of
    /// the previous media, or before the end of the prefix.
    fn stream_media(&mut self, records: &[MountedEntry], order: &[usize]) -> Result<Vec<Vec<u8>>> {
        let mut media = vec![Vec::new(); records.len()];
        let mut position = self.bytes.len() as u64;
        for &index in order {
            let record = &records[index];
            let offset = u64::from(record.offset);
            let size = u64::from(record.size);
            let gap = offset - position;
            let skipped = io::copy(&mut (&mut *self.reader).take(gap), &mut io::sink())?;
            let mut data = Vec::with_capacity(to_usize(size.min(MAX_RESERVE))?);
            (&mut *self.reader).take(size).read_to_end(&mut data)?;
            if skipped != gap || data.len() as u64 != size {
                return Err(Error::EntryOutOfBounds {
                    name: record.name.clone(),
                });
            }
            media[index] = data;
            position = offset + size;
        }
        Ok(media)
    }

    /// Reads the input to its end and copies the media of `records` out of the bytes.
    fn buffer_media(&mut self, records: &[MountedEntry]) -> Result<Vec<Vec<u8>>> {
        self.reader.read_to_end(&mut self.bytes)?;
        records
            .iter()
            .map(|record| {
                let start = to_usize(u64::from(record.offset))?;
                let end = start + to_usize(u64::from(record.size))?;
                self.bytes
                    .get(start..end)
                    .map(<[u8]>::to_vec)
                    .ok_or_else(|| Error::EntryOutOfBounds {
                        name: record.name.clone(),
                    })
            })
            .collect()
    }
}

/// Reads the media of `entry` from `source`.
fn read_media(source: &mut (impl Read + Seek), entry: &MountedEntry) -> Result<Vec<u8>> {
    source.seek(SeekFrom::Start(u64::from(entry.offset)))?;
    let mut data = vec![0; to_usize(u64::from(entry.size))?];
    source.read_exact(&mut data)?;
    Ok(data)
}

/// The offsets of a package as [`Package::to_writer`] writes it.
struct Layout {
    /// The end of the offset table.
    table_end: u64,
    /// One slot per entry, in order.
    slots: Vec<Slot>,
}

/// The offsets of one entry.
struct Slot {
    record_offset: u32,
    data_offset: u32,
    /// The length of the name in UTF-16 code units.
    name_len: u32,
}

impl Layout {
    /// Returns the layout of section 5.1 of the spec for `entries`.
    ///
    /// Each record starts at the first 8-byte boundary after the previous record. The data region
    /// starts at the first 8-byte boundary after the last record. Each media starts at the first
    /// 8-byte boundary after the previous media.
    fn of(entries: &[PackageEntry]) -> Result<Self> {
        let table_end = HEADER_LEN + 4 * entries.len() as u64;
        let mut record_end = table_end;
        let mut slots = Vec::with_capacity(entries.len());
        for entry in entries {
            let name_len = entry.name.encode_utf16().count() as u64;
            let record_offset = record_end.next_multiple_of(ALIGNMENT);
            record_end = record_offset + RECORD_HEADER_LEN + 2 * name_len;
            slots.push(Slot {
                record_offset: to_u32(record_offset, "package record offset")?,
                data_offset: 0,
                name_len: to_u32(name_len, "package entry name length")?,
            });
        }
        let mut data_end = record_end;
        for (entry, slot) in entries.iter().zip(&mut slots) {
            let data_offset = data_end.next_multiple_of(ALIGNMENT);
            data_end = data_offset + entry.data.len() as u64;
            slot.data_offset = to_u32(data_offset, "package media offset")?;
        }
        to_u32(data_end, "package length")?;
        Ok(Self { table_end, slots })
    }
}

/// Writes zeros from `position` to `target` and returns `target`.
fn pad(writer: &mut impl Write, position: u64, target: u64) -> Result<u64> {
    io::copy(&mut io::repeat(0).take(target - position), writer)?;
    Ok(target)
}

/// Converts an offset or a size of the package to a `usize`.
fn to_usize(value: u64) -> io::Result<usize> {
    usize::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("package offset or size {value} exceeds usize::MAX"),
        )
    })
}
