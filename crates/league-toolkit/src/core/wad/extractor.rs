use std::{
    collections::HashMap,
    io::{Read, Seek},
    path::Path,
};

use crate::core::wad::WadHashtableExt;

use super::{Wad, WadDecoder, WadError, WadHashtable};

impl<TSource: Read + Seek> Wad<TSource> {
    pub fn extract_all(
        &mut self,
        output_dir: impl AsRef<Path>,
        hashtable: &WadHashtable,
    ) -> Result<(), WadError> {
        let (mut decoder, chunks) = self.decode();
        for chunk in chunks.values() {
            let data = decoder.load_chunk_decompressed(chunk)?;

            let chunk_path = hashtable.resolve_or_default(chunk.path_hash);

            println!("Chunk: {:?}", chunk);
        }

        Ok(())
    }
}
