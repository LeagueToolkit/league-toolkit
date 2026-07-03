//! Reading, writing, and extracting League of Legends WAD archives.
//!
//! # Reading a WAD
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_wad::Wad;
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//!
//! // Iterate chunks in path-hash order
//! for chunk in wad.chunks() {
//!     println!("{:#016x} ({} bytes)", chunk.path_hash(), chunk.uncompressed_size());
//! }
//!
//! // Read and decompress a specific chunk
//! let chunk = *wad.chunks().get(0x1234567890abcdef).unwrap();
//! let data = wad.load_chunk_decompressed(&chunk)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Extracting to disk
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_wad::{Wad, WadExtractor, HexPathResolver};
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//!
//! let extractor = WadExtractor::new(&HexPathResolver)
//!     .on_progress(|p| println!("{:.0}%", p.percent() * 100.0));
//!
//! let count = extractor.extract_all(&mut wad, "/output/path")?;
//! println!("Extracted {count} chunks");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Parallel decompression
//!
//! [`decompress_raw`] decouples decompression from I/O, so you can stream raw
//! reads sequentially and dispatch decompression to a thread pool:
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_wad::{Wad, WadChunk, decompress_raw};
//!
//! let file = File::open("archive.wad.client")?;
//! let mut wad = Wad::mount(file)?;
//! let chunks: Vec<WadChunk> = wad.chunks().iter().copied().collect();
//!
//! // Sequential read, parallel decompress + process.
//! // With rayon you would send (chunk, raw) into a parallel iterator;
//! // here we show the sequential equivalent.
//! for chunk in &chunks {
//!     let raw = wad.load_chunk_raw(chunk)?;
//!
//!     // This can run on another thread — no borrow on `wad` needed.
//!     let decompressed = decompress_raw(
//!         &raw,
//!         chunk.compression_type(),
//!         chunk.uncompressed_size(),
//!     )?;
//!
//!     // ... write `decompressed` to disk, process it, etc.
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Building a WAD
//!
//! ```no_run
//! use std::io::{Cursor, Write};
//! use ltk_wad::{WadBuilder, WadChunkBuilder};
//!
//! let mut builder = WadBuilder::default();
//! builder = builder.with_chunk(WadChunkBuilder::default().with_path("path/to/asset"));
//!
//! let mut output = Cursor::new(Vec::new());
//! builder.build_to_writer(&mut output, |_path_hash, cursor| {
//!     cursor.write_all(b"asset data")?;
//!     Ok(())
//! })?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod builder;
mod chunk;
mod chunks;
mod decoder;
mod error;
mod extractor;
mod file_ext;

pub use builder::*;
pub use chunk::*;
pub use chunks::*;
pub use decoder::*;
pub use error::*;
pub use extractor::*;
pub use file_ext::*;

use std::io::{BufReader, Read, Seek, SeekFrom};

use byteorder::{ReadBytesExt as _, LE};
use sha2::{Digest as _, Sha256};

pub use rsa;

/// Riot's WAD signing public key, DER-encoded (SubjectPublicKeyInfo).
pub const RITO_PKEY: &[u8] = b"\x30\x82\x01\x22\x30\x0d\x06\x09\x2a\x86\x48\x86\xf7\x0d\x01\x01\x01\x05\x00\x03\x82\x01\x0f\x00\x30\x82\x01\x0a\x02\x82\x01\x01\x00\xcc\xcc\xf2\xa9\x77\x5f\x5e\x48\x30\x4f\x4c\x99\x8e\xa3\x05\x74\x14\xf9\x41\xaf\xd2\xd3\x82\x8e\x38\x0e\xb8\x3e\x50\x49\xc3\x56\x78\xba\x3d\x63\xc2\x3c\x88\x3d\x5e\xbc\xbb\x26\x59\x92\xc7\xb1\x84\x46\x2c\x25\xea\xc5\x19\x27\xa4\xd4\x93\xd0\xea\x65\xdf\x4d\xda\xe1\x34\xfe\xbd\x10\xf0\x6d\x4f\x9c\x02\xce\x83\x18\x15\xf9\x56\x5d\x86\x59\xe9\x01\x5c\xd0\x48\x22\xd7\x09\xaa\x37\x35\xd9\xdb\x7f\x9d\xa1\xd9\x6c\xa2\x31\x70\x46\x65\x54\xc6\xd4\x8c\x22\x4f\x73\x87\x83\x56\xd0\xa3\x56\xde\x41\x9b\x48\x7e\xcb\xa1\x1c\x18\xb0\xbd\xa3\x64\xa1\xea\xb7\x8f\x2e\x4c\x9e\x42\x40\x99\x86\x70\x97\x31\x92\xcf\xe4\x90\x36\x7f\xf7\x2b\x9d\x26\xd1\x9d\x00\x05\xfd\x08\xcf\xf3\x2c\x55\x8d\xff\x31\xf0\xc0\xa5\xce\xb2\xf5\x2b\xd4\x28\xd7\x91\xa7\x00\x81\xae\x6e\xac\xd1\xaf\xf9\x1c\x91\x9d\x7c\xf3\x8a\xce\x25\xd4\x19\x60\xb5\x50\x0e\x55\xb8\xb2\x01\x01\x9f\xfc\xff\xba\x27\x6f\xa0\xf7\xb7\x92\xe4\x6b\xc2\x2f\x40\xa3\x2f\x24\x44\xab\xc4\xb6\xe2\xf7\xe8\x90\x65\xc8\x84\xaf\x2a\x00\xee\x3d\xa5\xbb\x8a\xf2\xea\x2a\xde\x92\x6c\xf2\xa1\x48\xd7\x02\x03\x01\x00\x01";

// serde has no built-in impls for arrays longer than 32 elements.
#[cfg(feature = "serde")]
mod signature_serde {
    use serde::de::Error as _;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(
        signature: &[u8; 256],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        signature[..].serialize(serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 256], D::Error> {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        let len = bytes.len();
        bytes
            .try_into()
            .map_err(|_| D::Error::invalid_length(len, &"256 bytes"))
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug)]
/// A wad file
pub struct Wad<TSource: Read + Seek> {
    chunks: WadChunks,
    #[cfg_attr(feature = "serde", serde(with = "signature_serde"))]
    signature: [u8; 256],
    checksum: u64,
    #[cfg_attr(feature = "serde", serde(skip))]
    source: TSource,
}

impl<TSource: Read + Seek> Wad<TSource> {
    pub fn chunks(&self) -> &WadChunks {
        &self.chunks
    }

    pub fn mount(mut source: TSource) -> Result<Wad<TSource>, WadError> {
        let mut reader = BufReader::new(&mut source);

        // 0x5752 = "RW"
        let magic = reader.read_u16::<LE>()?;
        if magic != 0x5752 {
            return Err(WadError::InvalidHeader {
                expected: String::from("RW"),
                actual: format!("0x{:x}", magic),
            });
        }

        let major = reader.read_u8()?;
        let minor = reader.read_u8()?;
        if major > 3 {
            return Err(WadError::InvalidVersion { major, minor });
        }

        let (signature, checksum) = if major == 2 {
            let mut ecdsa_signature = [0u8; 256];
            reader.read_exact(&mut ecdsa_signature[..84])?;
            let data_checksum = reader.read_u64::<LE>()?;
            (ecdsa_signature, data_checksum)
        } else if major == 3 {
            let mut pkcs1_signature = [0u8; 256];
            reader.read_exact(&mut pkcs1_signature)?;
            let data_checksum = reader.read_u64::<LE>()?;
            (pkcs1_signature, data_checksum)
        } else {
            ([0u8; 256], 0u64)
        };

        if major == 1 || major == 2 {
            let _toc_start_offset = reader.seek(SeekFrom::Current(2))?;
            let _toc_chunk_size = reader.seek(SeekFrom::Current(2))?;
        }

        let chunk_count = reader.read_i32::<LE>()? as usize;
        let mut raw_chunks = Vec::<WadChunk>::with_capacity(chunk_count);
        for _ in 0..chunk_count {
            let chunk = match (major, minor) {
                (3, 0..=3) => WadChunk::read_v3_1(&mut reader),
                (3, _) => WadChunk::read_v3_4(&mut reader),
                _ => Err(WadError::InvalidVersion { major, minor }),
            }?;

            raw_chunks.push(chunk);
        }

        let chunks = WadChunks::from_iter(raw_chunks);

        Ok(Wad {
            signature,
            checksum,
            chunks,
            source,
        })
    }

    /// Consumes the `Wad`, returning the underlying source and chunks.
    ///
    /// This enables callers to take ownership of both parts separately,
    /// e.g. to wrap the source in synchronization primitives or to create
    /// multiple readers for parallel extraction.
    pub fn into_parts(self) -> (TSource, WadChunks) {
        (self.source, self.chunks)
    }

    /// Reads the raw (compressed) bytes of a chunk from the source.
    pub fn load_chunk_raw(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        let mut data = vec![0; chunk.compressed_size];

        self.source
            .seek(SeekFrom::Start(chunk.data_offset as u64))?;
        self.source.read_exact(&mut data)?;

        Ok(data.into_boxed_slice())
    }

    /// Reads and decompresses a chunk from the source.
    pub fn load_chunk_decompressed(&mut self, chunk: &WadChunk) -> Result<Box<[u8]>, WadError> {
        let raw_data = self.load_chunk_raw(chunk)?;
        decompress_raw(&raw_data, chunk.compression_type, chunk.uncompressed_size)
    }

    /// Returns embedded checksum verbatim.
    pub fn checksum(&self) -> u64 {
        self.checksum
    }

    /// Returns embedded signature.
    pub fn signature(&self) -> &[u8; 256] {
        &self.signature
    }

    /// Computes SHA-256 of the TOC.
    ///
    /// The TOC bytes are reconstructed from the parsed chunks in v3.4 format,
    /// in path-hash order — byte-identical to the TOC of a v3.4 file.
    pub fn toc_sha256(&self) -> Result<[u8; 32], WadError> {
        let mut hasher = Sha256::new();
        for chunk in self.chunks.iter() {
            chunk.write_v3_4(&mut hasher)?;
        }
        Ok(hasher.finalize().into())
    }

    /// Verifies the embedded signature as PKCS#1 v1.5 over SHA-256 of the TOC.
    ///
    /// Returns whether the signature is valid for `public_key`, along with
    /// the computed TOC SHA-256.
    pub fn verify_signature(
        &self,
        public_key: &rsa::RsaPublicKey,
    ) -> Result<(bool, [u8; 32]), WadError> {
        let toc_sha256 = self.toc_sha256()?;
        let valid = public_key
            .verify(
                rsa::Pkcs1v15Sign::new::<Sha256>(),
                &toc_sha256,
                &self.signature,
            )
            .is_ok();
        Ok((valid, toc_sha256))
    }
}
