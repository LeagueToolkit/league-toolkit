//! Rewriting a WAD in place, keeping the bytes that did not change.
//!
//! The crate root documents what a rebase is and the layout the tail strategy
//! reads; this module is the mechanism.
//!
//! # Why the strategy is named at the constructor
//!
//! "Rebase a WAD" is the general operation, and appending past the kept region
//! is merely the cheapest way to do it, so the strategy has to be visible
//! somewhere. It is named by the constructor - [`WadRebasePlan::tail`] - rather
//! than by a `kind` field or an enum with one variant today.
//!
//! Each strategy decides where the bytes go, so each wants a record of its own
//! geometry: [`WadTailLayout`] describes `[header][TOC][region][tail]`, and a
//! strategy that placed its bytes elsewhere would describe something else. A
//! constructor takes its own strategy's record as an argument, which a shared
//! kind field cannot express - it would have to widen to a union of every
//! strategy's layout, and [`write`](WadRebasePlan::write) would match on a
//! discriminant to find out which one it holds. Naming the strategy at the
//! constructor also keeps the interface at what a caller must learn now: one
//! function, no enum, and a second strategy arrives as a sibling of `tail`
//! rather than as a variant every existing caller has to reckon with.
//!
//! The same reasoning is why the layout keeps a tail-specific name while the
//! plan, the report and the error do not.

use std::collections::BTreeMap;
use std::io::{BufWriter, Seek, SeekFrom, Write};
use std::sync::Arc;

use byteorder::{WriteBytesExt as _, LE};
use xxhash_rust::xxh3::xxh3_64;

use crate::{WadChunk, WadChunkCompression, WadError, WadHash};

#[cfg(test)]
mod tests;

/// Size of a single v3.4 WAD TOC entry.
const TOC_ENTRY_SIZE: usize = 32;

/// Size of the v3.4 WAD header, which every other offset in the file follows.
///
/// 2 bytes of `RW` magic, 2 of version, 256 of RSA signature and 8 of checksum.
/// The `u32` chunk count sits directly on top of it and the TOC directly on top
/// of that, so the first TOC entry of a well-formed v3.4 WAD begins at 272.
const WAD_HEADER_SIZE: u64 = 268;

/// Write buffer size for a rebased WAD.
const WRITE_BUFFER_SIZE: usize = 1 << 20; // 1 MiB

/// TOC entries reserved beyond a rebasable WAD's current entry count.
///
/// Zero, deliberately. Reserving slack would let a WAD gain or lose a chunk
/// without moving any data, but it leaves a gap between the last TOC entry and
/// the first data byte, and the game has not been observed tolerating that gap
/// in a real session. The capacity is still recorded and honoured throughout,
/// and a rebase zeroes the slots it leaves unfilled, so enabling slack once
/// that is proven is this constant alone.
///
/// While it is zero, capacity equals the entry count, so any change to a WAD's
/// entry set fails the capacity precondition and leaves the caller its full
/// rebuild - which also means nothing exercises the zero-fill until this is
/// raised.
const TOC_SLACK_ENTRIES: u32 = 0;

/// Highest byte offset the WAD v3.4 format's `u32` offset fields can address.
const MAX_WAD_OFFSET: u64 = u32::MAX as u64;

/// Why a WAD could not be rebased.
///
/// Everything a rebase alone can refuse, named without reference to where the
/// WAD lives: it is handed a seekable target and a base offset, so it has no
/// path to put in a message. A caller that has one attaches it.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WadRebaseError {
    /// The recorded layout does not place its regions past a header, a chunk
    /// count and a TOC of the recorded capacity.
    #[error(
        "a layout reserving {toc_capacity} TOC entries cannot put its data region at \
         {data_region_offset} and its tail at {tail_offset}"
    )]
    IncoherentLayout {
        /// TOC entries the layout reserved.
        toc_capacity: u32,
        /// Where the layout puts the data region it keeps.
        data_region_offset: u64,
        /// Where the layout puts the tail.
        tail_offset: u64,
    },

    /// The rewritten TOC no longer fits the capacity the file reserved.
    #[error("the WAD reserved {reserved} TOC entries, not the {needed} this rebase needs")]
    TocCapacity {
        /// Entries the rewritten TOC would hold.
        needed: usize,
        /// Entries the file has room for.
        reserved: u32,
    },

    /// The tail would reach past what the format's `u32` offsets can address.
    #[error(
        "the tail would reach offset {offset}, past the 4 GiB the WAD v3.4 format \
         can address"
    )]
    TailTooLarge {
        /// The offset the tail would have reached.
        offset: u64,
    },

    /// A kept chunk's shifted range falls outside the addressable file.
    #[error("chunk {path_hash:016x} cannot be addressed at offset {offset}")]
    ChunkUnaddressable {
        /// The chunk that would not fit.
        path_hash: WadHash,
        /// Where shifting would have put it.
        offset: i64,
    },

    /// A TOC entry could not be encoded.
    ///
    /// Encoding one only ever fails by failing to write it, so in practice this
    /// is [`WadError::IoError`] and a second way for the target to give out. A
    /// caller that treats I/O specially has to look through this variant as
    /// well as [`Io`](Self::Io).
    #[error(transparent)]
    Wad(#[from] WadError),

    /// The target refused a write or a seek.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// What a rebase wrote.
///
/// The numbers the write itself produced, and nothing the caller already held:
/// the layout it passed in, the source it verified and the time it chose to
/// measure are all its own to report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WadRebaseReport {
    /// Entries in the rewritten TOC.
    pub entry_count: usize,
    /// Bytes the rewritten tail occupies.
    pub tail_len: u64,
}

/// A chunk's encoded bytes plus the TOC fields that describe them.
///
/// What a rebase writes: bytes already under a WAD codec, ready to go into the
/// file verbatim. Building one hashes the bytes, so the checksum a TOC entry
/// carries can never disagree with the bytes it points at - which matters,
/// because the game kills the process over a chunk whose checksum does not
/// match its content.
///
/// Cloning is cheap: the bytes live behind an [`Arc`], so one encoding can be
/// shared by every WAD it is written into. That sharing is also what keeps a
/// chunk that appears in several archives byte-identical in all of them, which
/// is how the game validates a shared chunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedChunk {
    compressed: Arc<[u8]>,
    uncompressed_size: u32,
    compression: WadChunkCompression,
    /// xxh3_64 of `compressed`, the chunk's TOC checksum field.
    checksum: u64,
}

impl EncodedChunk {
    /// Take `compressed` as a chunk body already encoded with `compression`.
    ///
    /// `uncompressed_size` is what the bytes decode to, and for a
    /// [`None`](WadChunkCompression::None) chunk is their own length: a TOC
    /// where the two disagree makes the client read past the buffer it
    /// allocated for the chunk.
    ///
    /// The checksum is computed here rather than accepted, so a caller cannot
    /// pass on a value some container claimed. Recomputing runs at about memcpy
    /// speed, so the copy stays the cost.
    ///
    /// A [`ZstdMulti`](WadChunkCompression::ZstdMulti) body cannot be rebased.
    /// Its bytes are a run of subchunks that only decode alongside the
    /// [`SubchunkToc`](crate::SubchunkToc) records naming them, which live in
    /// the archive rather than in the chunk, so the entry a rebase writes for
    /// it - subchunk fields zeroed, because it has no records here - is one the
    /// game cannot resolve. Which codecs a caller emits is the caller's policy,
    /// so this takes the format's full [`WadChunkCompression`] and does not
    /// refuse; a caller that copies chunks out of an existing archive is the
    /// one that has to drop the `ZstdMulti` ones.
    pub fn new(
        compressed: impl Into<Arc<[u8]>>,
        uncompressed_size: u32,
        compression: WadChunkCompression,
    ) -> Self {
        let compressed = compressed.into();
        Self {
            checksum: xxh3_64(&compressed),
            compressed,
            uncompressed_size,
            compression,
        }
    }

    /// The encoded bytes, written into the WAD verbatim.
    #[must_use]
    pub fn compressed(&self) -> &[u8] {
        &self.compressed
    }

    /// Size of the chunk once decoded.
    #[must_use]
    pub fn uncompressed_size(&self) -> u32 {
        self.uncompressed_size
    }

    /// Codec [`compressed`](Self::compressed) is encoded with.
    #[must_use]
    pub fn compression(&self) -> WadChunkCompression {
        self.compression
    }

    /// xxh3_64 of the encoded bytes - the chunk's TOC checksum.
    #[must_use]
    pub fn checksum(&self) -> u64 {
        self.checksum
    }
}

/// Where a tail-rebasable WAD's regions sit, and how source offsets map into it.
///
/// Recorded when the file is laid out so a later rebase can verify the file it
/// finds on disk and rewrite only its tail. See the [crate root](crate) for
/// the layout this describes.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WadTailLayout {
    /// Absolute offset where the kept data region starts.
    pub data_region_offset: u64,
    /// Added to a source chunk's `data_offset` to get its offset in this file.
    ///
    /// Signed: this file's TOC can be shorter than the source's own when the
    /// source WAD pads between its TOC and its first chunk.
    pub offset_delta: i64,
    /// Absolute offset where the tail starts (the region's end).
    pub tail_offset: u64,
    /// TOC entries the file has room for without moving any data.
    ///
    /// Equal to the entry count: the crate reserves no slack beyond it today.
    pub toc_capacity: u32,
}

impl WadTailLayout {
    /// Absolute offset of the first TOC entry.
    ///
    /// Derived by subtracting the TOC from the region offset rather than from
    /// the header size, so where the region sits stays the single fact the rest
    /// of the layout is read off.
    ///
    /// # Errors
    ///
    /// Fails when the layout does not place its region past a header, a chunk
    /// count and a TOC of the recorded capacity. Every field here may have been
    /// deserialized from a record that anything could have written, and the
    /// result is a seek target in a file the game loads, so the subtraction
    /// reports rather than wraps.
    pub fn toc_offset(&self) -> Result<u64, WadRebaseError> {
        self.chunk_count_offset()
            .map(|offset| offset + size_of::<u32>() as u64)
    }

    /// Absolute offset of the `u32` chunk count, which precedes the TOC.
    ///
    /// # Errors
    ///
    /// Fails on the same layouts as [`toc_offset`](Self::toc_offset), of which
    /// this is the lower bound.
    pub fn chunk_count_offset(&self) -> Result<u64, WadRebaseError> {
        let below_region =
            u64::from(self.toc_capacity) * TOC_ENTRY_SIZE as u64 + size_of::<u32>() as u64;
        self.data_region_offset
            .checked_sub(below_region)
            .filter(|&offset| offset >= WAD_HEADER_SIZE)
            .ok_or(WadRebaseError::IncoherentLayout {
                toc_capacity: self.toc_capacity,
                data_region_offset: self.data_region_offset,
                tail_offset: self.tail_offset,
            })
    }

    /// A source chunk's TOC entry with its offset moved into the kept region.
    ///
    /// Every other field carries over untouched: the bytes came out of a valid
    /// v3.4 WAD and were copied verbatim, so their sizes, compression, frame
    /// fields and checksum still describe them exactly.
    ///
    /// # Errors
    ///
    /// Fails when the shifted range falls outside what the format's `u32`
    /// offset fields can address.
    pub fn shifted(&self, orig: &WadChunk) -> Result<WadChunk, WadRebaseError> {
        let shifted = orig.data_offset as i64 + self.offset_delta;
        let end = shifted + orig.compressed_size as i64;
        if shifted < 0 || end > MAX_WAD_OFFSET as i64 {
            return Err(WadRebaseError::ChunkUnaddressable {
                path_hash: orig.path_hash,
                offset: shifted,
            });
        }

        Ok(WadChunk {
            data_offset: shifted as usize,
            ..*orig
        })
    }

    /// Whether `entry_count` entries fit this TOC without an unreserved gap.
    ///
    /// The count may not exceed the capacity, nor fall short of it by more than
    /// the slack the crate reserves. That slack is zero today, because the game
    /// has not been observed tolerating the gap it would leave between the last
    /// TOC entry and the first data byte, so this is currently equality.
    #[must_use]
    pub fn admits_entry_count(&self, entry_count: usize) -> bool {
        let fewest = self.toc_capacity.saturating_sub(TOC_SLACK_ENTRIES);
        u32::try_from(entry_count).is_ok_and(|count| (fewest..=self.toc_capacity).contains(&count))
    }

    /// Check the layout's own numbers hang together before its offsets are used.
    ///
    /// A layout may come back from a record that anything could have written,
    /// and [`toc_offset`](Self::toc_offset) subtracts the TOC's size from the
    /// region offset to produce a seek target inside a file the game will load.
    /// Callers that got a layout from anywhere but a fresh build must run this
    /// first.
    ///
    /// # Errors
    ///
    /// Fails when the region does not start past a header, a chunk count and a
    /// TOC of the recorded capacity, or when the tail starts before the region
    /// does.
    pub fn validate(&self) -> Result<(), WadRebaseError> {
        self.chunk_count_offset()?;
        if self.tail_offset < self.data_region_offset {
            return Err(WadRebaseError::IncoherentLayout {
                toc_capacity: self.toc_capacity,
                data_region_offset: self.data_region_offset,
                tail_offset: self.tail_offset,
            });
        }
        Ok(())
    }
}

/// A checked plan to rebase a WAD: rewrite some of its chunks and its TOC.
///
/// The file keeps the region the strategy does not touch - the bytes that
/// dominate a game WAD - so the work is bounded by the changed bytes, not by
/// the archive's size.
///
/// A plan is built by the constructor for the strategy it uses;
/// [`tail`](Self::tail) is the one the crate has today. Building it performs
/// every check that can be made without touching the WAD, and
/// [`write`](Self::write) then consumes it. That split is the whole point of
/// the type: rewriting in place is destructive, so a caller has to commit to it
/// before a byte is written, by truncating a file or growing an entry inside a
/// container, and a plan is the proof that committing is safe.
/// [`tail_len`](Self::tail_len) reports how far a container's own bytes move,
/// which such a caller also needs before it starts.
///
/// Everything a plan needs to be *correct* has been decided by the caller: the
/// layout and base entries describe a WAD whose identity and TOC it has already
/// verified.
#[derive(Debug)]
pub struct WadRebasePlan<'a> {
    layout: WadTailLayout,
    /// The TOC the write emits. A tail chunk overwrites its entry here, so a
    /// chunk whose replacement is gone reverts to the bytes the region still
    /// holds. Consumed and written back out, which for a map WAD is tens of
    /// thousands of entries not worth copying.
    entries: BTreeMap<WadHash, WadChunk>,
    tail: &'a [(WadHash, EncodedChunk)],
    entry_count: usize,
    tail_end: u64,
}

impl<'a> WadRebasePlan<'a> {
    /// Check that `tail` can be appended to the WAD `layout` describes.
    ///
    /// The tail strategy, which is the cheapest rebase: the kept region stays
    /// where it is and the changed chunks go past it. See the
    /// [crate root](crate).
    ///
    /// # Arguments
    ///
    /// * `layout` - The layout recorded when the WAD was laid out.
    /// * `base_entries` - The TOC entry each chunk already in the kept region
    ///   would have with nothing rewritten, keyed by path hash.
    /// * `tail` - The chunks whose bytes go into the new tail, in the order
    ///   they are written. Their order decides where the bytes land and nothing
    ///   else: each TOC entry carries its own offset, and the TOC itself comes
    ///   out in hash order whatever order the tail was given in. A hash may
    ///   appear once.
    ///
    /// # Errors
    ///
    /// Fails when the recorded layout is incoherent, when the resulting entry
    /// count no longer fits the reserved TOC capacity, or when the tail would
    /// push the WAD past the format's 4 GiB limit. The caller's fallback for
    /// all of these is a full rebuild - and because nothing has been written,
    /// it inherits an untouched WAD.
    pub fn tail(
        layout: &WadTailLayout,
        base_entries: BTreeMap<WadHash, WadChunk>,
        tail: &'a [(WadHash, EncodedChunk)],
    ) -> Result<Self, WadRebaseError> {
        layout.validate()?;

        let entry_count =
            Self::merged_entry_count(&base_entries, tail.iter().map(|(hash, _)| *hash));
        if !layout.admits_entry_count(entry_count) {
            return Err(WadRebaseError::TocCapacity {
                needed: entry_count,
                reserved: layout.toc_capacity,
            });
        }

        // Saturating rather than checked: any sum large enough to wrap a u64 is
        // far past the 4 GiB limit the next check rejects it by, so there is
        // nothing a separate overflow error would tell the caller.
        let tail_end = tail.iter().fold(layout.tail_offset, |end, (_, chunk)| {
            end.saturating_add(chunk.compressed().len() as u64)
        });
        if tail_end > MAX_WAD_OFFSET {
            return Err(WadRebaseError::TailTooLarge { offset: tail_end });
        }

        Ok(Self {
            layout: *layout,
            entries: base_entries,
            tail,
            entry_count,
            tail_end,
        })
    }

    /// How many TOC entries a base entry set plus a set of tail hashes comes to.
    ///
    /// A tail hash that is already a base entry replaces it rather than adding
    /// one, which is what lets a chunk whose replacement was removed revert to
    /// the region's bytes without changing the WAD's shape. This is the count
    /// [`tail`](Self::tail) checks against the layout's capacity, exposed so a
    /// caller can run the same precheck before it assembles a tail.
    #[must_use]
    pub fn merged_entry_count(
        base_entries: &BTreeMap<WadHash, WadChunk>,
        tail_hashes: impl IntoIterator<Item = WadHash>,
    ) -> usize {
        base_entries.len()
            + tail_hashes
                .into_iter()
                .filter(|hash| !base_entries.contains_key(hash))
                .count()
    }

    /// Bytes the rewritten tail will occupy.
    #[must_use]
    pub fn tail_len(&self) -> u64 {
        self.tail_end - self.layout.tail_offset
    }

    /// Entries the rewritten TOC will hold.
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entry_count
    }

    /// Write the tail and the TOC into `wad`, whose first byte is at `base`.
    ///
    /// `base` is where the WAD begins inside whatever holds it: `0` for a file
    /// that is one WAD, and the entry's offset for a WAD stored inside an
    /// archive. It shifts every seek and no recorded offset - a TOC entry
    /// counts from the WAD's own first byte, because the game reads the WAD
    /// without knowing what it is stored in.
    ///
    /// The caller owns the container, so the caller makes room: a file is
    /// truncated to the layout's tail offset before this is called, and a WAD
    /// inside an archive has its entry grown or shrunk by the difference
    /// [`tail_len`](Self::tail_len) reports.
    ///
    /// # Errors
    ///
    /// Fails when the target refuses a write or a seek, or when a TOC entry
    /// cannot be encoded. Every check that does not need the target was made
    /// when the plan was built, so a failure here means the target itself gave
    /// out - and, the write being in place, may leave a torn WAD. That is what
    /// a caller's dirty marker exists to cover.
    pub fn write<W: Write + Seek>(
        mut self,
        wad: &mut W,
        base: u64,
    ) -> Result<WadRebaseReport, WadRebaseError> {
        let mut writer = BufWriter::with_capacity(WRITE_BUFFER_SIZE, wad);
        writer.seek(SeekFrom::Start(base + self.layout.tail_offset))?;

        let mut cursor = self.layout.tail_offset;
        for (path_hash, encoded) in self.tail {
            let chunk = Self::write_tail_chunk(&mut writer, *path_hash, encoded, &mut cursor)?;
            self.entries.insert(*path_hash, chunk);
        }
        let tail_len = cursor - self.layout.tail_offset;

        // `entries` is a BTreeMap, so this walks the TOC in ascending hash order.
        writer.seek(SeekFrom::Start(base + self.layout.chunk_count_offset()?))?;
        writer.write_u32::<LE>(self.entries.len() as u32)?;
        for chunk in self.entries.values() {
            chunk.write_v3_4(&mut writer)?;
        }
        // Reserved slots this rebase did not fill are zeroed, as a full rebuild
        // zeroes them: rewriting in place would otherwise leave the previous
        // TOC's entries sitting past the new chunk count. Empty while
        // `TOC_SLACK_ENTRIES` is zero, since capacity then equals the entry count.
        for _ in self.entries.len()..self.layout.toc_capacity as usize {
            writer.write_all(&[0u8; TOC_ENTRY_SIZE])?;
        }

        writer.flush()?;

        Ok(WadRebaseReport {
            entry_count: self.entries.len(),
            tail_len,
        })
    }

    /// Append one encoded chunk to the tail and return the TOC entry for it.
    ///
    /// Advances `cursor` past the bytes written, so the offset bookkeeping the
    /// tail depends on lives in exactly one place.
    ///
    /// # Errors
    ///
    /// Fails when the chunk would end past what the format's `u32` offset
    /// fields can address, which building the plan already ruled out.
    fn write_tail_chunk<W: Write>(
        writer: &mut W,
        path_hash: WadHash,
        encoded: &EncodedChunk,
        cursor: &mut u64,
    ) -> Result<WadChunk, WadRebaseError> {
        let compressed_size = encoded.compressed().len();
        let end = *cursor + compressed_size as u64;
        if end > MAX_WAD_OFFSET {
            return Err(WadRebaseError::TailTooLarge { offset: end });
        }

        let chunk = WadChunk {
            path_hash,
            data_offset: *cursor as usize,
            compressed_size,
            uncompressed_size: encoded.uncompressed_size() as usize,
            compression_type: encoded.compression(),
            // Not duplicated, and no subchunk records: a rebased chunk is one
            // run of bytes this write just laid down. A `ZstdMulti` body is
            // what these zeros make unresolvable - see [`EncodedChunk::new`].
            is_duplicated: false,
            frame_count: 0,
            start_frame: 0,
            checksum: encoded.checksum(),
        };

        writer.write_all(encoded.compressed())?;
        *cursor = end;

        Ok(chunk)
    }
}
