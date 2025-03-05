use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

use crate::{
    core::wad::{WadHashtableExt, WadHashtablePath},
    league_file::LeagueFileKind,
};

use super::{Wad, WadChunk, WadDecoder, WadError, WadHashtable};

impl<TSource: Read + Seek> Wad<TSource> {
    /// Extract all chunks from the WAD file to the given directory.
    ///
    /// # Arguments
    ///
    /// * `output_dir` - The directory to extract the chunks to.
    /// * `hashtable` - The hashtable to use to resolve the chunk paths.
    pub fn extract_all(
        &mut self,
        output_dir: impl AsRef<Path>,
        hashtable: &WadHashtable,
    ) -> Result<(), WadError> {
        let (mut decoder, chunks) = self.decode();
        for chunk in chunks.values() {
            Self::extract_chunk(chunk, &output_dir, &mut decoder, hashtable)?;
        }

        Ok(())
    }

    /// Extract a chunk from the WAD file to the given directory.
    pub fn extract_chunk(
        chunk: &WadChunk,
        output_dir: impl AsRef<Path>,
        decoder: &mut WadDecoder<TSource>,
        hashtable: &WadHashtable,
    ) -> Result<(), WadError> {
        let data = decoder.load_chunk_decompressed(chunk)?;

        let chunk_path = hashtable.resolve_or_default(chunk.path_hash);

        let output_path = match chunk_path {
            WadHashtablePath::Default(path) => {
                let path = Path::new(path);
                let dir = output_dir.as_ref().join(path.parent().unwrap());
                std::fs::create_dir_all(&dir)?;

                output_dir.as_ref().join(path)
            }
            WadHashtablePath::Unknown(name) => {
                let name = Self::resolve_unknown_chunk_name(&name, &data);

                output_dir.as_ref().join(&name)
            }
        };

        let mut file = File::create(output_path)?;
        file.write_all(&data)?;

        Ok(())
    }

    fn resolve_unknown_chunk_name(name: &str, data: &[u8]) -> PathBuf {
        let path = Path::new(name);
        let extension = path.extension().unwrap_or_default();

        let kind = match LeagueFileKind::from_extension(extension.to_str().unwrap_or_default()) {
            LeagueFileKind::Unknown => LeagueFileKind::identify_from_bytes(data),
            kind => kind,
        };

        match kind {
            // If the final kind is unknown, prefix with dot
            LeagueFileKind::Unknown => {
                let mut path = PathBuf::new();
                path.push(".");
                path.push(name);
                path
            }
            _ => path.with_extension(kind.extension().unwrap_or_default()),
        }
    }
}
