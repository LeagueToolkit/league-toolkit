use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};

use byteorder::{WriteBytesExt, LE};
use flate2::read::GzEncoder;
use io_ext::measure;
use itertools::Itertools;
use xxhash_rust::{xxh3, xxh64};

use crate::league_file::LeagueFileKind;

use super::{WadChunk, WadChunkCompression, WadError};

#[derive(Debug, thiserror::Error)]
pub enum WadBuilderError {
    #[error("wad error")]
    WadError(#[from] WadError),

    #[error("io error")]
    IoError(#[from] io::Error),

    #[error("unsupported compression type: {0}")]
    UnsupportedCompressionType(WadChunkCompression),
}

/// Implements a builder interface for creating WAD files.
///
/// # Examples
/// ```
/// # use league_toolkit::core::wad::*;
/// #
/// let builder = WadBuilder::default();
/// builder.with_chunk(WadChunkBuilder::default().with_path("path/to/chunk"));
/// builder.build_to_writer(File::create("output.wad").unwrap());
/// ```
#[derive(Debug, Default)]
pub struct WadBuilder {
    chunk_builders: Vec<WadChunkBuilder>,
}

impl WadBuilder {
    pub fn with_chunk(mut self, chunk: WadChunkBuilder) -> Self {
        self.chunk_builders.push(chunk);
        self
    }

    /// Build the WAD file and write it to the given writer.
    ///
    /// * `writer` - The writer to write the WAD file to.
    /// * `provide_chunk_data` - A function that provides the rawdata for each chunk.
    pub fn build_to_writer<
        TWriter: io::Write + io::Seek,
        TChunkDataProvider: Fn(u64, &mut Cursor<&mut Vec<u8>>) -> Result<(), WadBuilderError>,
    >(
        self,
        writer: TWriter,
        provide_chunk_data: TChunkDataProvider,
    ) -> Result<(), WadBuilderError> {
        // First we need to write a dummy header and TOC, so we can calculate from where to start writing the chunks
        let mut writer = BufWriter::new(writer);

        let (header_toc_size, toc_offset) = self.write_dummy_toc::<TWriter>(&mut writer)?;

        let ordered_chunks = self
            .chunk_builders
            .iter()
            .sorted_by_key(|chunk| chunk.path)
            .collect::<Vec<_>>();

        let mut final_chunks = Vec::new();

        let mut current_data_offset = toc_offset + header_toc_size;
        for chunk in ordered_chunks {
            let mut chunk_data = Vec::new();
            provide_chunk_data(chunk.path, &mut Cursor::new(&mut chunk_data))?;

            let chunk_data_size = chunk_data.len();
            let compressed_data = Self::compress_chunk_data(&chunk_data, chunk.force_compression)?;
            let compressed_data_size = compressed_data.len();
            let compressed_checksum = xxh3::xxh3_64(&compressed_data);

            writer.write_all(&compressed_data)?;

            current_data_offset = current_data_offset.wrapping_add(compressed_data_size as u64);

            final_chunks.push(WadChunk {
                path_hash: chunk.path,
                data_offset: current_data_offset as usize,
                compressed_size: compressed_data_size,
                uncompressed_size: chunk_data_size,
                compression_type: WadChunkCompression::Zstd,
                is_duplicated: false,
                frame_count: 0,
                start_frame: 0,
                checksum: compressed_checksum,
            });
        }

        writer.seek(SeekFrom::Start(toc_offset))?;

        for chunk in final_chunks {
            chunk.write_v3_4(&mut writer)?;
        }

        Ok(())
    }

    fn write_dummy_toc<W: io::Write + io::Seek>(
        &self,
        writer: &mut BufWriter<W>,
    ) -> Result<(u64, u64), WadBuilderError> {
        let (header_toc_size, toc_offset) = measure(writer, |writer| {
            // Write the header
            writer.write_u16::<LE>(0x5752)?;
            writer.write_u8(3)?; // major
            writer.write_u8(4)?; // minor

            // Write dummy ECDSA signature
            writer.write_all(&[0; 256])?;
            writer.write_u64::<LE>(0)?;

            // Write dummy TOC
            let toc_offset = writer.stream_position()?;
            for _ in self.chunk_builders.iter() {
                writer.write_all(&[0; 32])?;
            }

            Ok::<_, WadBuilderError>(toc_offset)
        })?;

        Ok((header_toc_size, toc_offset))
    }

    fn compress_chunk_data(
        data: &[u8],
        force_compression: Option<WadChunkCompression>,
    ) -> Result<Vec<u8>, WadBuilderError> {
        let compressed_data = match force_compression {
            Some(compression) => Self::compress_chunk_data_by_compression(data, compression)?,
            None => {
                let kind = LeagueFileKind::identify_from_bytes(data);

                Self::compress_chunk_data_by_compression(data, kind.ideal_compression())?
            }
        };

        Ok(compressed_data)
    }

    fn compress_chunk_data_by_compression(
        data: &[u8],
        compression: WadChunkCompression,
    ) -> Result<Vec<u8>, WadBuilderError> {
        let mut compressed_data = Vec::new();
        match compression {
            WadChunkCompression::None => {
                compressed_data = data.to_vec();
            }
            WadChunkCompression::GZip => {
                let reader = BufReader::new(data);
                let mut encoder = GzEncoder::new(reader, flate2::Compression::default());

                encoder.read_to_end(&mut compressed_data)?;
            }
            WadChunkCompression::Zstd => {
                zstd::Encoder::new(BufWriter::new(&mut compressed_data), 3)?.write_all(data)?
            }
            WadChunkCompression::Satellite => {
                return Err(WadBuilderError::UnsupportedCompressionType(compression));
            }
            WadChunkCompression::ZstdMulti => {
                return Err(WadBuilderError::UnsupportedCompressionType(compression));
            }
        }

        Ok(compressed_data)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WadChunkBuilder {
    path: u64,

    /// If provided, the chunk will be compressed using the given compression type, otherwise the ideal compression will be used.
    force_compression: Option<WadChunkCompression>,
}

impl WadChunkBuilder {
    pub fn with_path(mut self, path: impl AsRef<str>) -> Self {
        self.path = xxh64::xxh64(path.as_ref().to_lowercase().as_bytes(), 0);
        self
    }

    pub fn with_force_compression(mut self, compression: WadChunkCompression) -> Self {
        self.force_compression = Some(compression);
        self
    }
}
