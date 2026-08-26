//! The subchunk table that describes [`ZstdMulti`](crate::WadChunkCompression::ZstdMulti) chunks.

use crate::{decompress_raw, WadChunk, WadChunkCompression, WadChunks, WadError};

/// One record of a WAD's subchunk table.
///
/// A `ZstdMulti` chunk is a run of subchunks laid end to end. A record holds
/// one subchunk's sizes; equal sizes mean the subchunk is stored raw, anything
/// else is one zstd frame.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WadSubchunk {
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    /// XXH3 of the subchunk's compressed bytes.
    pub checksum: u64,
}

impl WadSubchunk {
    /// Whether the subchunk is stored raw rather than as a zstd frame.
    pub fn is_raw(&self) -> bool {
        self.compressed_size == self.uncompressed_size
    }
}

/// A WAD's subchunk table, read out of its `*.SubChunkTOC` chunk.
///
/// A [`WadChunk`] names its subchunks by index: `frame_count` records
/// starting at `start_frame`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SubchunkToc {
    records: Box<[WadSubchunk]>,
}

/// The size of one table record on disk.
const RECORD_SIZE: usize = 16;

impl SubchunkToc {
    /// Parses a table from the decompressed bytes of a `SubChunkTOC` chunk.
    ///
    /// # Errors
    ///
    /// Fails when the length is not a multiple of the 16-byte record size.
    pub fn from_bytes(data: &[u8]) -> Result<Self, WadError> {
        if !data.len().is_multiple_of(RECORD_SIZE) {
            return Err(WadError::Other(format!(
                "subchunk toc length {} is not a multiple of {RECORD_SIZE}",
                data.len()
            )));
        }
        let records = data
            .chunks_exact(RECORD_SIZE)
            .map(|record| WadSubchunk {
                compressed_size: u32::from_le_bytes(record[0..4].try_into().unwrap()),
                uncompressed_size: u32::from_le_bytes(record[4..8].try_into().unwrap()),
                checksum: u64::from_le_bytes(record[8..16].try_into().unwrap()),
            })
            .collect();
        Ok(Self { records })
    }

    /// All records, in frame order.
    pub fn records(&self) -> &[WadSubchunk] {
        &self.records
    }

    /// The records of `chunk`'s subchunks, in the order they are laid out.
    ///
    /// `None` for a chunk that is not `ZstdMulti`, or whose frames the table
    /// does not hold.
    pub fn subchunks_of(&self, chunk: &WadChunk) -> Option<&[WadSubchunk]> {
        if chunk.compression_type != WadChunkCompression::ZstdMulti {
            return None;
        }
        let start = chunk.start_frame as usize;
        self.records.get(start..start + chunk.frame_count as usize)
    }

    /// Whether the table describes every `ZstdMulti` chunk of `chunks`.
    ///
    /// True only when each such chunk's records exist and their sizes sum to
    /// the chunk's, which no unrelated chunk's bytes pass by accident.
    pub fn covers(&self, chunks: &WadChunks) -> bool {
        chunks
            .iter()
            .filter(|chunk| chunk.compression_type == WadChunkCompression::ZstdMulti)
            .all(|chunk| {
                self.subchunks_of(chunk).is_some_and(|subchunks| {
                    let compressed: u64 =
                        subchunks.iter().map(|sub| sub.compressed_size as u64).sum();
                    let uncompressed: u64 = subchunks
                        .iter()
                        .map(|sub| sub.uncompressed_size as u64)
                        .sum();
                    compressed == chunk.compressed_size as u64
                        && uncompressed == chunk.uncompressed_size as u64
                })
            })
    }
}

/// Finds the table's chunk by shape: sixteen bytes per frame, records summing
/// to every `ZstdMulti` chunk's sizes.
///
/// By shape because the chunk's own name hashes the archive's install path
/// with its extension as `SubChunkTOC`, which a mounted stream does not know.
pub(crate) fn detect_subchunk_toc(
    chunks: &WadChunks,
    mut load: impl FnMut(&WadChunk) -> Result<Box<[u8]>, WadError>,
) -> Option<SubchunkToc> {
    let frames = chunks
        .iter()
        .filter(|chunk| chunk.compression_type == WadChunkCompression::ZstdMulti)
        .map(|chunk| chunk.start_frame as usize + chunk.frame_count as usize)
        .max()?;
    let candidates = chunks
        .iter()
        .filter(|chunk| chunk.uncompressed_size == frames * RECORD_SIZE);
    for candidate in candidates {
        let Ok(raw) = load(candidate) else { continue };
        let Ok(data) = decompress_raw(
            &raw,
            candidate.compression_type,
            candidate.uncompressed_size,
        ) else {
            continue;
        };
        let Ok(toc) = SubchunkToc::from_bytes(&data) else {
            continue;
        };
        if toc.covers(chunks) {
            return Some(toc);
        }
    }
    None
}

#[cfg(test)]
mod tests;
