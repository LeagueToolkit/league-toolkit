//! The format information of a WEM file.
//!
//! A WEM is a RIFF file with the form type `WAVE`. [`WemInfo`] reads its `fmt ` and `data` chunk
//! headers. The crate does not decode audio.

use std::time::Duration;

use crate::{Error, Result};

/// The format of a WEM, from its `fmt ` chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WemInfo {
    /// The `wFormatTag` of the `fmt ` chunk.
    pub codec: WemCodec,
    /// The `nChannels` of the `fmt ` chunk.
    pub channels: u16,
    /// The `nSamplesPerSec` of the `fmt ` chunk.
    pub sample_rate: u32,
    /// The `nAvgBytesPerSec` of the `fmt ` chunk.
    pub average_bytes_per_second: u32,
    /// The size of the `data` chunk.
    pub data_size: u32,
}

impl WemInfo {
    /// Reads the `fmt ` and `data` chunk headers of the RIFF file `data`.
    ///
    /// The walk starts after the 12-byte RIFF header and reads chunks in sequence until it has
    /// both. Each chunk body is padded to an even size. The audio is not read.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Wem`] when `data` does not start with `RIFF`, has no `WAVE` form type,
    /// has no `fmt ` or no `data` chunk, has a `fmt ` chunk shorter than 16 bytes or past the end
    /// of `data`, or ends inside a chunk header.
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.get(0..4) != Some(b"RIFF") {
            return Err(Error::Wem("no RIFF header"));
        }
        if data.get(8..12) != Some(b"WAVE") {
            return Err(Error::Wem("no WAVE form type"));
        }

        let mut format: Option<[u8; FMT_LEN]> = None;
        let mut data_size: Option<u32> = None;
        let mut offset = RIFF_HEADER_LEN;
        loop {
            if let (Some(format), Some(data_size)) = (format, data_size) {
                return Ok(Self::from_fmt(&format, data_size));
            }
            let rest = data.get(offset..).unwrap_or_default();
            if rest.is_empty() {
                return Err(Error::Wem(if format.is_none() {
                    "no fmt chunk"
                } else {
                    "no data chunk"
                }));
            }
            let Some((header, body)) = rest.split_first_chunk::<CHUNK_HEADER_LEN>() else {
                return Err(Error::Wem("file ends inside a chunk header"));
            };
            let size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
            match &header[..4] {
                b"fmt " if format.is_none() => {
                    let fmt = usize::try_from(size)
                        .ok()
                        .and_then(|size| body.get(..size))
                        .ok_or(Error::Wem("fmt chunk ends after the end of the file"))?
                        .first_chunk::<FMT_LEN>()
                        .ok_or(Error::Wem("fmt chunk shorter than 16 bytes"))?;
                    format = Some(*fmt);
                }
                b"data" if data_size.is_none() => data_size = Some(size),
                _ => {}
            }
            // The body is padded to an even size. A size past the end of `data` ends the walk.
            let padded = u64::from(size) + u64::from(size & 1);
            offset = usize::try_from(padded)
                .ok()
                .and_then(|padded| (offset + CHUNK_HEADER_LEN).checked_add(padded))
                .unwrap_or(usize::MAX);
        }
    }

    /// Returns `data_size / average_bytes_per_second`, or `None` when the rate is 0.
    ///
    /// The duration is exact for PCM and an estimate for the compressed codecs.
    pub fn duration(&self) -> Option<Duration> {
        let rate = u64::from(self.average_bytes_per_second);
        if rate == 0 {
            return None;
        }
        // At most (2^32 - 1) * 10^9 < 2^64.
        Some(Duration::from_nanos(
            u64::from(self.data_size) * NANOS_PER_SECOND / rate,
        ))
    }

    fn from_fmt(fmt: &[u8; FMT_LEN], data_size: u32) -> Self {
        Self {
            codec: WemCodec(u16::from_le_bytes([fmt[0], fmt[1]])),
            channels: u16::from_le_bytes([fmt[2], fmt[3]]),
            sample_rate: u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]),
            average_bytes_per_second: u32::from_le_bytes([fmt[8], fmt[9], fmt[10], fmt[11]]),
            data_size,
        }
    }
}

/// The `wFormatTag` of a WEM.
///
/// A WEM carries tags that the crate does not name. The constants name the tags of the Wwise
/// codecs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WemCodec(pub u16);

impl WemCodec {
    /// Integer PCM.
    pub const PCM: Self = Self(0x0001);
    /// IMA ADPCM.
    pub const ADPCM: Self = Self(0x0002);
    /// Opus.
    pub const OPUS: Self = Self(0x3040);
    /// Wwise Opus.
    pub const WWISE_OPUS: Self = Self(0x3041);
    /// PCM with a `WAVEFORMATEXTENSIBLE` format chunk.
    pub const PCM_EXTENSIBLE: Self = Self(0xFFFE);
    /// Wwise Vorbis.
    pub const VORBIS: Self = Self(0xFFFF);
}

/// The size of the RIFF header: `RIFF`, the `u32` size and the form type.
const RIFF_HEADER_LEN: usize = 12;

/// The size of a chunk header: the tag and the `u32` size.
const CHUNK_HEADER_LEN: usize = 8;

/// The leading fields of `fmt `: `wFormatTag`, `nChannels`, `nSamplesPerSec`, `nAvgBytesPerSec`,
/// `nBlockAlign` and `wBitsPerSample`.
const FMT_LEN: usize = 16;

const NANOS_PER_SECOND: u64 = 1_000_000_000;
