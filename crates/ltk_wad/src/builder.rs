use std::io::{self, BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};

use byteorder::{WriteBytesExt, LE};
use flate2::read::GzEncoder;
use itertools::Itertools;
use ltk_io_ext::measure;
use xxhash_rust::{xxh3, xxh64};

use ltk_file::LeagueFileKind;

use crate::FileExt as _;

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
/// ## This example builds a WAD file in memory
/// ```
/// # use ltk_wad::*;
/// # use std::io::{Cursor, Write};
///
/// let mut builder = WadBuilder::default();
/// let scratch = Vec::new();
/// let mut wad_cursor = Cursor::new(scratch);
///
/// builder = builder.with_chunk(WadChunkBuilder::default().with_path("path/to/chunk"));
/// builder.build_to_writer(&mut wad_cursor, |path, cursor| {
///     cursor.write_all(&[0xAA; 100])?;
///
///     Ok(())
/// })
/// .expect("Failed to build WAD");
/// ```
#[derive(Debug)]
pub struct WadBuilder {
    chunk_builders: Vec<WadChunkBuilder>,
    signature: [u8; 256],
    checksum: u64,
}

impl Default for WadBuilder {
    fn default() -> Self {
        Self {
            chunk_builders: Default::default(),
            signature: [0u8; 256],
            checksum: Default::default(),
        }
    }
}

impl WadBuilder {
    pub fn with_chunk(mut self, chunk: WadChunkBuilder) -> Self {
        self.chunk_builders.push(chunk);
        self
    }

    pub fn with_checksum(mut self, checksum: u64) -> Self {
        self.checksum = checksum;
        self
    }

    pub fn with_signature(mut self, signature: &[u8; 256]) -> Self {
        self.signature.copy_from_slice(signature);
        self
    }

    /// Build the WAD file and write it to the given writer.
    ///
    /// * `writer` - The writer to write the WAD file to.
    /// * `provide_chunk_data` - A function that provides the rawdata for each chunk.
    pub fn build_to_writer<
        TWriter: io::Write + io::Seek,
        TChunkDataProvider: Fn(u64, &mut Cursor<Vec<u8>>) -> Result<(), WadBuilderError>,
    >(
        self,
        writer: &mut TWriter,
        provide_chunk_data: TChunkDataProvider,
    ) -> Result<(), WadBuilderError> {
        // First we need to write a dummy header and TOC, so we can calculate from where to start writing the chunks
        let mut writer = BufWriter::new(writer);

        let (_, toc_offset) = self.write_dummy_toc::<TWriter>(&mut writer)?;

        // Sort the chunks by path hash, otherwise League wont load the WAD
        let ordered_chunks = self
            .chunk_builders
            .iter()
            .sorted_by_key(|chunk| chunk.path)
            .collect::<Vec<_>>();

        let mut final_chunks = Vec::new();

        for chunk in ordered_chunks {
            let mut cursor = Cursor::new(Vec::new());
            provide_chunk_data(chunk.path, &mut cursor)?;

            let chunk_data_size = cursor.get_ref().len();
            let (compressed_data, compression) =
                Self::compress_chunk_data(cursor.get_ref(), chunk.force_compression)?;
            let compressed_data_size = compressed_data.len();
            let compressed_checksum = xxh3::xxh3_64(&compressed_data);

            let chunk_data_offset = writer.stream_position()?;
            writer.write_all(&compressed_data)?;

            final_chunks.push(WadChunk {
                path_hash: chunk.path,
                data_offset: chunk_data_offset as usize,
                compressed_size: compressed_data_size,
                uncompressed_size: chunk_data_size,
                compression_type: compression,
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
        writer: &mut BufWriter<&mut W>,
    ) -> Result<(u64, u64), WadBuilderError> {
        let (header_toc_size, toc_offset) = measure(writer, |writer| {
            // Write the header
            writer.write_u16::<LE>(0x5752)?;
            writer.write_u8(3)?; // major
            writer.write_u8(4)?; // minor

            // Write signature and checksum verbatim.
            writer.write_all(&self.signature)?;
            writer.write_u64::<LE>(self.checksum)?;

            // Write dummy TOC
            writer.write_u32::<LE>(self.chunk_builders.len() as u32)?;
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
    ) -> Result<(Vec<u8>, WadChunkCompression), WadBuilderError> {
        let (compressed_data, compression) = match force_compression {
            Some(compression) => (
                Self::compress_chunk_data_by_compression(data, compression)?,
                compression,
            ),
            None => {
                let kind = LeagueFileKind::identify_from_bytes(data);
                let compression = kind.ideal_compression();
                let compressed_data = Self::compress_chunk_data_by_compression(data, compression)?;

                (compressed_data, compression)
            }
        };

        Ok((compressed_data, compression))
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
                #[cfg(feature = "zstd")]
                {
                    let mut encoder = zstd::Encoder::new(BufWriter::new(&mut compressed_data), 3)?;
                    encoder.write_all(data)?;
                    encoder.finish()?;
                }
                #[cfg(feature = "ruzstd")]
                {
                    ruzstd::encoding::compress(
                        data,
                        &mut compressed_data,
                        ruzstd::encoding::CompressionLevel::Fastest,
                    );
                }
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

/// Implements a builder interface for creating WAD chunks.
///
/// # Examples
/// ```
/// # use ltk_wad::*;
/// #
/// let builder = WadChunkBuilder::default();
/// builder.with_path("path/to/chunk");
/// builder.with_force_compression(WadChunkCompression::Zstd);
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct WadChunkBuilder {
    /// The path hash of the chunk. Hashed using xxhash64.
    path: u64,

    /// If provided, the chunk will be compressed using the given compression type, otherwise the ideal compression will be used.
    force_compression: Option<WadChunkCompression>,
}

impl WadChunkBuilder {
    /// Set the chunk path hash by hashing the given str.
    ///
    /// If you already have the hash itself, see [`with_hash`](Self::with_hash).
    pub fn with_path(mut self, path: impl AsRef<str>) -> Self {
        self.path = xxh64::xxh64(path.as_ref().to_lowercase().as_bytes(), 0);
        self
    }

    /// Set the chunk path hash directly.
    ///
    /// If you have the actual path instead of the hash, see [`with_path`](Self::with_path)
    pub fn with_hash(mut self, hash: u64) -> Self {
        self.path = hash;
        self
    }

    pub fn with_force_compression(mut self, compression: WadChunkCompression) -> Self {
        self.force_compression = Some(compression);
        self
    }
}

#[cfg(test)]
mod tests {
    use crate::Wad;
    use sha2::{Digest as _, Sha256};

    use super::*;

    #[test]
    fn test_wad_builder() {
        let scratch = Vec::new();
        let mut cursor = Cursor::new(scratch);

        let mut builder = WadBuilder::default()
            .with_signature(&[0xAB; 256])
            .with_checksum(0xDEADBEEFCAFEBABE);
        builder = builder.with_chunk(WadChunkBuilder::default().with_path("test1"));
        builder = builder.with_chunk(WadChunkBuilder::default().with_path("test2"));
        builder = builder.with_chunk(WadChunkBuilder::default().with_path("test3"));

        builder
            .build_to_writer(&mut cursor, |_path, cursor| {
                cursor.write_all(&[0xAA; 100])?;

                Ok(())
            })
            .expect("Failed to build WAD");

        cursor.set_position(0);
        let built_bytes = cursor.get_ref().clone();

        let wad = Wad::mount(cursor).expect("Failed to mount WAD");
        assert_eq!(wad.chunks().len(), 3);
        assert_eq!(wad.signature(), &[0xAB; 256]);
        assert_eq!(wad.checksum(), 0xDEADBEEFCAFEBABE);

        // Header layout: magic (2) + version (2), signature (256), checksum (8),
        // chunk count (4), then the TOC (32 bytes per chunk).
        assert_eq!(built_bytes[4..260], [0xABu8; 256][..]);
        assert_eq!(built_bytes[260..268], 0xDEADBEEFCAFEBABEu64.to_le_bytes());
        let toc = &built_bytes[272..272 + 3 * 32];
        let toc_sha256: [u8; 32] = Sha256::digest(toc).into();
        assert_eq!(wad.toc_sha256().unwrap(), toc_sha256);

        let chunk = wad.chunks().get(xxh64::xxh64(b"test1", 0)).unwrap();
        assert_eq!(chunk.path_hash, xxh64::xxh64(b"test1", 0));
        assert_eq!(chunk.compressed_size, 17);
        assert_eq!(chunk.uncompressed_size, 100);
        assert_eq!(chunk.compression_type, WadChunkCompression::Zstd);
    }

    #[test]
    fn test_wad_signature_verify() {
        use rsa::{Pkcs1v15Sign, RsaPrivateKey};

        let mut rng = rand::thread_rng();
        let private_key = RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate key");
        let public_key = private_key.to_public_key();

        let build = |signature: &[u8; 256]| {
            let mut cursor = Cursor::new(Vec::new());
            WadBuilder::default()
                .with_signature(signature)
                .with_chunk(WadChunkBuilder::default().with_path("test1"))
                .with_chunk(WadChunkBuilder::default().with_path("test2"))
                .build_to_writer(&mut cursor, |_path, cursor| {
                    cursor.write_all(&[0xAA; 100])?;

                    Ok(())
                })
                .expect("Failed to build WAD");
            cursor.set_position(0);
            Wad::mount(cursor).expect("Failed to mount WAD")
        };

        // The TOC depends only on chunk data, so hash an unsigned build,
        // sign it, and rebuild with the signature.
        let unsigned = build(&[0u8; 256]);
        let toc_sha256 = unsigned.toc_sha256().unwrap();
        let signature = private_key
            .sign(Pkcs1v15Sign::new::<sha2::Sha256>(), &toc_sha256)
            .expect("Failed to sign");

        let signed = build(signature.as_slice().try_into().unwrap());
        let (valid, computed) = signed.verify_signature(&public_key).unwrap();
        assert!(valid);
        assert_eq!(computed, toc_sha256);

        let (valid, computed) = unsigned.verify_signature(&public_key).unwrap();
        assert!(!valid);
        assert_eq!(computed, toc_sha256);
    }
}
