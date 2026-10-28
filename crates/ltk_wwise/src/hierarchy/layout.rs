//! The payload reader and writer that the typed layouts share.
//!
//! A layout reads a payload with a [`Reader`] and stops with a [`Stop`]. The caller turns a
//! [`Stop`] into an [`Error`] that names the object.

use crate::{bank::BankVersion, Error, ObjectId, Result};

use super::{to_u32, ObjectKind};

/// A set of bank versions that share one set of payload layouts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Generation {
    /// Version 132 and version 134.
    Legacy,
    /// Version 145.
    Current,
}

impl Generation {
    /// Returns the generation of `version`.
    ///
    /// # Errors
    ///
    /// - [`Error::UnsupportedLayout`] for a version other than 132, 134 and 145.
    pub(crate) fn of(version: BankVersion) -> Result<Self> {
        match version {
            BankVersion::V132 | BankVersion::V134 => Ok(Self::Legacy),
            BankVersion::V145 => Ok(Self::Current),
            _ => Err(Error::UnsupportedLayout(version)),
        }
    }

    /// Returns `true` for the current generation.
    pub(crate) fn is_current(self) -> bool {
        self == Self::Current
    }
}

/// The point at which a layout stops reading a payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Stop {
    /// The layout ends at `offset`, or needs the payload up to `offset`.
    At(usize),
    /// An action class outside the table of the generation.
    Class(u16),
}

impl Stop {
    /// Returns the error of a stop in the payload of `len` bytes of the object `kind` `id`.
    pub(crate) fn into_error(self, kind: ObjectKind, id: ObjectId, len: usize) -> Error {
        match self {
            Self::At(offset) => Error::Layout {
                kind,
                id,
                offset,
                len,
            },
            Self::Class(class) => Error::ActionClass { id, class },
        }
    }
}

/// The result of a layout step.
pub(crate) type Step<T> = std::result::Result<T, Stop>;

/// Returns the length of `count` records of `size` bytes, saturated at `usize::MAX`.
fn records(count: u32, size: usize) -> usize {
    (count as usize).saturating_mul(size)
}

/// A cursor over a payload. The position never passes the end of the payload.
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// Returns a reader at the start of `data`.
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Returns the next `len` bytes.
    pub(crate) fn take(&mut self, len: usize) -> Step<&'a [u8]> {
        let end = self.pos.saturating_add(len);
        let bytes = self.data.get(self.pos..end).ok_or(Stop::At(end))?;
        self.pos = end;
        Ok(bytes)
    }

    /// Returns the next `N` bytes.
    pub(crate) fn array<const N: usize>(&mut self) -> Step<[u8; N]> {
        let end = self.pos.saturating_add(N);
        let (bytes, _) = self
            .data
            .get(self.pos..)
            .and_then(<[u8]>::split_first_chunk::<N>)
            .ok_or(Stop::At(end))?;
        self.pos = end;
        Ok(*bytes)
    }

    /// Skips `len` bytes.
    pub(crate) fn skip(&mut self, len: usize) -> Step<()> {
        self.take(len).map(drop)
    }

    /// Skips `count` records of `size` bytes.
    pub(crate) fn skip_records(&mut self, count: u32, size: usize) -> Step<()> {
        self.skip(records(count, size))
    }

    pub(crate) fn u8(&mut self) -> Step<u8> {
        self.array::<1>().map(|[byte]| byte)
    }

    pub(crate) fn u16(&mut self) -> Step<u16> {
        self.array().map(u16::from_le_bytes)
    }

    pub(crate) fn u32(&mut self) -> Step<u32> {
        self.array().map(u32::from_le_bytes)
    }

    pub(crate) fn id(&mut self) -> Step<ObjectId> {
        self.u32().map(ObjectId)
    }

    /// Reads a 7-bit big-endian varint.
    ///
    /// Each byte contributes its low 7 bits, most significant group first. A byte with bit 7
    /// clear ends the value. A value above `u32::MAX` stops at the byte that overflows.
    pub(crate) fn var(&mut self) -> Step<u32> {
        let mut value: u32 = 0;
        loop {
            let at = self.pos;
            let byte = self.u8()?;
            value = value.checked_mul(0x80).ok_or(Stop::At(at))? | u32::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
    }

    /// Reads `count` object IDs.
    pub(crate) fn ids(&mut self, count: u32) -> Step<Vec<ObjectId>> {
        let (ids, _) = self.take(records(count, 4))?.as_chunks::<4>();
        Ok(ids
            .iter()
            .map(|id| ObjectId(u32::from_le_bytes(*id)))
            .collect())
    }

    /// Skips a `PropBundle`: a `u8` count, `count` `u8` IDs and `count` `u32` values.
    pub(crate) fn prop_bundle(&mut self) -> Step<()> {
        let count = self.u8()?;
        self.skip_records(u32::from(count), 1 + 4)
    }

    /// Skips a `RangedPropBundle`: a `u8` count, `count` `u8` IDs and `count` `u32` pairs.
    pub(crate) fn ranged_prop_bundle(&mut self) -> Step<()> {
        let count = self.u8()?;
        self.skip_records(u32::from(count), 1 + 8)
    }

    /// Skips an `ExceptionList`: a varint count of `(u32 id, u8 is_bus)` records.
    pub(crate) fn exceptions(&mut self) -> Step<()> {
        let count = self.var()?;
        self.skip_records(count, 4 + 1)
    }

    /// Skips a `Positioning` block.
    ///
    /// Bits 0 and 1 of the first byte give a `u8` of 3D bits. A 3D automation mode of 1 or 2 in
    /// bits 5 and 6 gives the path data: a `u8` mode, a `u32` time, 16-byte vertices, 8-byte
    /// play list items and 12 bytes of automation parameters per play list item.
    pub(crate) fn positioning(&mut self) -> Step<()> {
        let bits = self.u8()?;
        if bits & 0b11 != 0b11 {
            return Ok(());
        }
        self.skip(1)?;
        if !matches!((bits >> 5) & 0b11, 1 | 2) {
            return Ok(());
        }
        self.skip(1 + 4)?;
        let vertices = self.u32()?;
        self.skip_records(vertices, 16)?;
        let items = self.u32()?;
        self.skip_records(items, 8 + 12)
    }

    /// Skips an `RtpcList`: a `u16` count of RTPCs, each with its graph points.
    pub(crate) fn rtpcs(&mut self) -> Step<()> {
        let count = self.u16()?;
        for _ in 0..count {
            self.skip(4 + 1 + 1)?;
            self.var()?;
            self.skip(4 + 1)?;
            let points = self.u16()?;
            self.skip_records(u32::from(points), 12)?;
        }
        Ok(())
    }

    /// Skips a `StateChunk`: the state groups, then the state properties.
    pub(crate) fn state_chunk(&mut self) -> Step<()> {
        let groups = self.var()?;
        for _ in 0..groups {
            self.var()?;
            self.skip(1 + 1)?;
        }
        let props = self.var()?;
        for _ in 0..props {
            self.skip(4 + 1)?;
            let values = self.var()?;
            self.skip_records(values, 4 + 4)?;
        }
        Ok(())
    }

    /// Runs `layout` and returns the bytes that it reads.
    pub(crate) fn bytes_of(&mut self, layout: impl FnOnce(&mut Self) -> Step<()>) -> Step<Vec<u8>> {
        let start = self.pos;
        layout(self)?;
        Ok(self.data[start..self.pos].to_vec())
    }

    /// Ends the layout: the payload ends at the position.
    pub(crate) fn finish(&self) -> Step<()> {
        if self.pos == self.data.len() {
            Ok(())
        } else {
            Err(Stop::At(self.pos))
        }
    }
}

/// Appends little-endian fields to a payload.
pub(crate) trait Put {
    fn put_u8(&mut self, value: u8);
    fn put_u16(&mut self, value: u16);
    fn put_u32(&mut self, value: u32);
    /// Appends `value` as a 7-bit big-endian varint of the fewest bytes.
    fn put_var(&mut self, value: u32);
    /// Appends a `u32` count of `len` items, and `what` names the count in the error.
    ///
    /// # Errors
    ///
    /// - [`Error::Io`] when `len` is larger than `u32::MAX`.
    fn put_count(&mut self, len: usize, what: &str) -> Result<u32>;
    /// Appends the object IDs `ids`.
    fn put_ids(&mut self, ids: &[ObjectId]);
}

impl Put for Vec<u8> {
    fn put_u8(&mut self, value: u8) {
        self.push(value);
    }

    fn put_u16(&mut self, value: u16) {
        self.extend_from_slice(&value.to_le_bytes());
    }

    fn put_u32(&mut self, value: u32) {
        self.extend_from_slice(&value.to_le_bytes());
    }

    fn put_var(&mut self, value: u32) {
        let mut groups = 1;
        while groups < 5 && value >> (7 * groups) != 0 {
            groups += 1;
        }
        for group in (0..groups).rev() {
            let bits = (value >> (7 * group)) as u8 & 0x7F;
            self.push(if group == 0 { bits } else { bits | 0x80 });
        }
    }

    fn put_count(&mut self, len: usize, what: &str) -> Result<u32> {
        let count = to_u32(len, what)?;
        self.put_u32(count);
        Ok(count)
    }

    fn put_ids(&mut self, ids: &[ObjectId]) {
        for id in ids {
            self.put_u32(id.0);
        }
    }
}
