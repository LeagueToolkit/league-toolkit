use std::io::{self, BufReader, BufWriter};

use xxhash_rust::xxh64;

use super::{WadChunk, WadChunkCompression, WadError};

pub enum WadBuilderError {
    WadError(WadError),
}

pub struct WadBuilder {
    chunk_builders: Vec<WadChunkBuilder>,
}

impl WadBuilder {
    pub fn new() -> Self {
        Self {
            chunk_builders: Vec::new(),
        }
    }

    pub fn with_chunk(mut self, chunk: WadChunkBuilder) -> Self {
        self.chunk_builders.push(chunk);
        self
    }

    pub fn build_to_writer<W: io::Write + io::Seek>(
        self,
        writer: W,
        provide_chunk_data: impl Fn(u64, &mut BufWriter<&mut [u8]>) -> Result<(), WadBuilderError>,
    ) -> Result<(), WadBuilderError> {
        todo!()

        // First we need to write a dummy header and TOC, so we can calculate from where to start writing the chunks
    }
}

pub struct WadChunkBuilder {
    path: u64,
    force_compression: Option<WadChunkCompression>,
}

impl WadChunkBuilder {
    pub fn new() -> Self {
        Self {
            path: 0,
            force_compression: None,
        }
    }

    pub fn with_path(mut self, path: impl AsRef<str>) -> Self {
        self.path = xxh64::xxh64(path.as_ref().to_lowercase().as_bytes(), 0);
        self
    }

    pub fn with_force_compression(mut self, compression: WadChunkCompression) -> Self {
        self.force_compression = Some(compression);
        self
    }
}
