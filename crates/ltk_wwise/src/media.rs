//! The media index of a bank: the `DIDX` and `DATA` pair.
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    mem,
    ops::Range,
};

use crate::{bank::write_chunk_header, Error, Result, SourceId};

/// The tag of the media index chunk.
pub(crate) const DIDX: [u8; 4] = *b"DIDX";

/// The tag of the media data chunk.
pub(crate) const DATA: [u8; 4] = *b"DATA";

/// The length of one `DIDX` record: source ID, offset and size.
const RECORD_LEN: usize = 12;

/// The media of a bank, by source ID.
///
/// A media index that the caller does not edit, and whose offsets as read are multiples of the
/// media alignment of the bank, writes its `DIDX` and `DATA` bodies as read. Two media indexes are equal when they hold the same media under the same source IDs.
#[derive(Debug, Clone)]
pub struct MediaIndex {
    repr: Repr,
}

/// Compares the media by source ID. The layout of the bodies as read is not compared.
impl PartialEq for MediaIndex {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}

impl Eq for MediaIndex {}

/// The storage of a media index.
#[derive(Debug, Clone)]
enum Repr {
    /// The bodies as read. `ranges` holds the span of each media in `data`.
    Read {
        didx: Vec<u8>,
        data: Vec<u8>,
        ranges: BTreeMap<SourceId, Range<usize>>,
    },
    /// Media that the writer lays out again.
    Owned(BTreeMap<SourceId, Vec<u8>>),
}

impl Default for MediaIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl MediaIndex {
    /// Returns an empty media index.
    pub fn new() -> Self {
        Self {
            repr: Repr::Owned(BTreeMap::new()),
        }
    }

    /// Returns the number of media.
    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::Read { ranges, .. } => ranges.len(),
            Repr::Owned(media) => media.len(),
        }
    }

    /// Returns `true` when the index holds no media.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns `true` when the index holds the media `source`.
    pub fn contains(&self, source: SourceId) -> bool {
        match &self.repr {
            Repr::Read { ranges, .. } => ranges.contains_key(&source),
            Repr::Owned(media) => media.contains_key(&source),
        }
    }

    /// Returns the bytes of the media `source`.
    pub fn get(&self, source: SourceId) -> Option<&[u8]> {
        match &self.repr {
            Repr::Read { data, ranges, .. } => ranges.get(&source).map(|r| &data[r.clone()]),
            Repr::Owned(media) => media.get(&source).map(Vec::as_slice),
        }
    }

    /// Returns the media in ascending source ID order.
    pub fn iter(&self) -> impl Iterator<Item = (SourceId, &[u8])> {
        let (read, owned) = match &self.repr {
            Repr::Read { data, ranges, .. } => (
                Some(ranges.iter().map(|(id, r)| (*id, &data[r.clone()]))),
                None,
            ),
            Repr::Owned(media) => (
                None,
                Some(media.iter().map(|(id, data)| (*id, data.as_slice()))),
            ),
        };
        read.into_iter()
            .flatten()
            .chain(owned.into_iter().flatten())
    }

    /// Inserts `data` as the media `source`. Returns the media it replaces.
    ///
    /// The bank writes an edited media index again: the records in ascending source ID order,
    /// each media on the media alignment of the bank.
    pub fn insert(&mut self, source: SourceId, data: Vec<u8>) -> Option<Vec<u8>> {
        self.owned_mut().insert(source, data)
    }

    /// Removes the media `source`. Returns the removed media.
    pub fn remove(&mut self, source: SourceId) -> Option<Vec<u8>> {
        if !self.contains(source) {
            return None;
        }
        self.owned_mut().remove(&source)
    }

    /// Returns the media as owned buffers. A read index copies its media out of `DATA`.
    fn owned_mut(&mut self) -> &mut BTreeMap<SourceId, Vec<u8>> {
        if let Repr::Read { data, ranges, .. } = &mut self.repr {
            let media = mem::take(ranges)
                .into_iter()
                .map(|(id, range)| (id, data[range].to_vec()))
                .collect();
            self.repr = Repr::Owned(media);
        }
        match &mut self.repr {
            Repr::Owned(media) => media,
            Repr::Read { .. } => unreachable!("a read media index converts to owned above"),
        }
    }

    /// Decodes the bodies of a `DIDX` chunk and the `DATA` chunk after it.
    ///
    /// The records are in any order. Bytes after the last whole record are kept.
    pub(crate) fn decode(didx: Vec<u8>, data: Vec<u8>) -> Result<Self> {
        let mut ranges = BTreeMap::new();
        for record in didx.as_chunks::<RECORD_LEN>().0 {
            let field = |at: usize| {
                u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]])
            };
            let source_id = SourceId(field(0));
            let start = u64::from(field(4));
            let end = start + u64::from(field(8));
            if end > data.len() as u64 {
                return Err(Error::MediaOutOfBounds { source_id });
            }
            // Both bounds are at most `data.len()`, a `usize`.
            let range = start as usize..end as usize;
            if ranges.insert(source_id, range).is_some() {
                return Err(Error::DuplicateSource(source_id));
            }
        }
        Ok(Self {
            repr: Repr::Read { didx, data, ranges },
        })
    }

    /// Returns the bodies as read when the writer writes them unchanged at `alignment`.
    ///
    /// The writer writes the bodies as read for an unedited index whose offsets as read are
    /// multiples of `alignment`.
    fn as_read(&self, alignment: u32) -> Option<(&[u8], &[u8])> {
        let Repr::Read { didx, data, ranges } = &self.repr else {
            return None;
        };
        let alignment = alignment.max(1) as usize;
        ranges
            .values()
            .all(|range| range.start.is_multiple_of(alignment))
            .then_some((didx.as_slice(), data.as_slice()))
    }

    /// Writes the `DIDX` and `DATA` chunks.
    ///
    /// An unedited index whose offsets are multiples of `alignment` writes its bodies as read.
    /// Every other index places its media on `alignment`.
    pub(crate) fn write(&self, writer: &mut impl Write, alignment: u32) -> Result<()> {
        match self.as_read(alignment) {
            Some((didx, data)) => {
                write_chunk_header(writer, DIDX, didx.len())?;
                writer.write_all(didx)?;
                write_chunk_header(writer, DATA, data.len())?;
                writer.write_all(data)?;
            }
            None => self.write_laid_out(writer, u64::from(alignment.max(1)))?,
        }
        Ok(())
    }

    /// Writes the media in ascending source ID order, each media at a multiple of `alignment`.
    fn write_laid_out(&self, writer: &mut impl Write, alignment: u64) -> Result<()> {
        let mut didx = Vec::with_capacity(self.len() * RECORD_LEN);
        let mut end = 0u64;
        for (source, data) in self.iter() {
            let offset = end.next_multiple_of(alignment);
            end = offset + data.len() as u64;
            for field in [source.0, to_u32(offset)?, to_u32(data.len() as u64)?] {
                didx.extend_from_slice(&field.to_le_bytes());
            }
        }
        write_chunk_header(writer, DIDX, didx.len())?;
        writer.write_all(&didx)?;

        write_chunk_header(writer, DATA, to_u32(end)? as usize)?;
        let mut written = 0u64;
        for (_, data) in self.iter() {
            let offset = written.next_multiple_of(alignment);
            io::copy(&mut io::repeat(0).take(offset - written), writer)?;
            writer.write_all(data)?;
            written = offset + data.len() as u64;
        }
        Ok(())
    }
}

/// Converts a `DATA` offset or size to its `u32` field.
fn to_u32(value: u64) -> Result<u32> {
    u32::try_from(value).map_err(|_| {
        Error::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("media offset {value} is larger than u32::MAX"),
        ))
    })
}
