use byteorder::{ReadBytesExt, LE};
use std::io::{Cursor, Read, Seek, SeekFrom};
use xxhash_rust::xxh64::xxh64;

use crate::defines::ShaderMacroDefinition;
use crate::error::{Result, ShaderError};
use crate::toc::ShaderToc;
use crate::{create_shader_bundle_path, create_shader_object_path, GraphicsPlatform, ShaderType};
use ltk_wad::Wad;

const SHADERS_PER_BUNDLE: u32 = 100;

pub struct ShaderLoader;

impl ShaderLoader {
    /// Loads the bytecode for a shader object from a WAD file by matching defines.
    /// # Arguments
    /// * `shader_object_path` - The path of the shader object to load.
    /// * `shader_type` - The type of the shader to load.
    /// * `platform` - The platform of the shader to load.
    /// * `defines` - The defines to use for the shader.
    /// * `wad` - The WAD file to load the shader from.
    /// # Errors
    /// Returns an error if the shader object is not found or if the shader object data cannot be read.
    pub fn load_bytecode<R: Read + Seek>(
        shader_object_path: &str,
        shader_type: ShaderType,
        platform: GraphicsPlatform,
        defines: &[ShaderMacroDefinition],
        wad: &mut Wad<R>,
    ) -> Result<Vec<u8>> {
        let toc = Self::load_toc(shader_object_path, shader_type, platform, wad)?;

        let filtered_defines_formatted = Self::filter_defines(defines, &toc.base_defines);
        let filtered_defines_hash = xxh64(filtered_defines_formatted.as_bytes(), 0);

        let shader_index = toc
            .shader_hashes
            .iter()
            .position(|&h| h == filtered_defines_hash)
            .ok_or(ShaderError::DefinesNotFound {
                defines: filtered_defines_formatted,
            })?;

        let shader_id = toc.shader_ids[shader_index];
        Self::load_bytecode_by_id(shader_object_path, shader_type, platform, shader_id, wad)
    }

    /// Loads the TOC for a shader object from a WAD file.
    pub fn load_toc<R: Read + Seek>(
        shader_object_path: &str,
        shader_type: ShaderType,
        platform: GraphicsPlatform,
        wad: &mut Wad<R>,
    ) -> Result<ShaderToc> {
        let full_shader_object_path =
            create_shader_object_path(shader_object_path, shader_type, platform);

        let path_hash = xxh64(full_shader_object_path.as_bytes(), 0);

        let chunk =
            *wad.chunks()
                .get(path_hash)
                .ok_or_else(|| ShaderError::ShaderObjectNotFound {
                    path: full_shader_object_path.clone(),
                })?;

        let shader_object_data = wad.load_chunk_decompressed(&chunk)?;
        let mut shader_object_reader = Cursor::new(shader_object_data);
        ShaderToc::read(&mut shader_object_reader)
    }

    /// Loads the bytecode for a single shader by its `shader_id` (as recorded in the TOC).
    pub fn load_bytecode_by_id<R: Read + Seek>(
        shader_object_path: &str,
        shader_type: ShaderType,
        platform: GraphicsPlatform,
        shader_id: u32,
        wad: &mut Wad<R>,
    ) -> Result<Vec<u8>> {
        let full_shader_object_path =
            create_shader_object_path(shader_object_path, shader_type, platform);

        let shader_bundle_id = SHADERS_PER_BUNDLE * (shader_id / SHADERS_PER_BUNDLE);
        let shader_index_in_bundle = shader_id % SHADERS_PER_BUNDLE;

        let bundle_data = Self::load_bundle_data(&full_shader_object_path, shader_bundle_id, wad)?;
        parse_bundle_entry_at(&bundle_data, shader_index_in_bundle)
    }

    /// Read a single bundle file and yield each entry's bytecode in order.
    /// Hot path for "dump every shader for one (object, type, platform)":
    /// reads the bundle chunk once instead of seeking N times.
    ///
    /// `full_shader_object_path` is the already-formatted path (e.g. from
    /// [`create_shader_object_path`]); `shader_bundle_id` is a multiple of 100.
    pub fn read_bundle<R: Read + Seek>(
        full_shader_object_path: &str,
        shader_bundle_id: u32,
        wad: &mut Wad<R>,
    ) -> Result<Vec<Vec<u8>>> {
        let bundle_data = Self::load_bundle_data(full_shader_object_path, shader_bundle_id, wad)?;
        parse_bundle_entries(&bundle_data)
    }

    fn load_bundle_data<R: Read + Seek>(
        full_shader_object_path: &str,
        shader_bundle_id: u32,
        wad: &mut Wad<R>,
    ) -> Result<Box<[u8]>> {
        let shader_bundle_path =
            create_shader_bundle_path(full_shader_object_path, shader_bundle_id);

        let bundle_path_hash = xxh64(shader_bundle_path.as_bytes(), 0);
        let bundle_chunk = *wad.chunks().get(bundle_path_hash).ok_or_else(|| {
            ShaderError::ShaderBundleNotFound {
                path: shader_bundle_path.clone(),
            }
        })?;

        Ok(wad.load_chunk_decompressed(&bundle_chunk)?)
    }

    /// Filters the defines to only include the defines that are in the base defines.
    fn filter_defines(
        defines: &[ShaderMacroDefinition],
        base_defines: &[ShaderMacroDefinition],
    ) -> String {
        let mut filtered = Vec::new();
        for req in defines {
            if base_defines.iter().any(|b| b.hash == req.hash) {
                filtered.push(req);
            }
        }

        filtered.sort_by(|a, b| a.name.cmp(&b.name));

        filtered.iter().map(|d| d.to_string()).collect()
    }
}

/// Parse a shader bundle blob into its constituent bytecode entries.
///
/// A bundle is a sequence of `(u32 length, [u8; length])` entries packed back-to-back.
fn parse_bundle_entries(data: &[u8]) -> Result<Vec<Vec<u8>>> {
    let total = data.len() as u64;
    let mut reader = Cursor::new(data);
    let mut entries = Vec::new();
    while reader.position() < total {
        let size = reader.read_u32::<LE>()? as usize;
        let mut bytecode = vec![0u8; size];
        reader.read_exact(&mut bytecode)?;
        entries.push(bytecode);
    }
    Ok(entries)
}

/// Parse a shader bundle blob and return only the entry at `index_in_bundle`.
fn parse_bundle_entry_at(data: &[u8], index_in_bundle: u32) -> Result<Vec<u8>> {
    let mut reader = Cursor::new(data);
    for _ in 0..index_in_bundle {
        let size = reader.read_u32::<LE>()?;
        reader.seek(SeekFrom::Current(i64::from(size)))?;
    }
    let size = reader.read_u32::<LE>()? as usize;
    let mut bytecode = vec![0u8; size];
    reader.read_exact(&mut bytecode)?;
    Ok(bytecode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use byteorder::WriteBytesExt;

    fn build_bundle(entries: &[&[u8]]) -> Vec<u8> {
        let mut out = Vec::new();
        for entry in entries {
            out.write_u32::<LE>(entry.len() as u32).unwrap();
            out.extend_from_slice(entry);
        }
        out
    }

    #[test]
    fn parse_bundle_entries_reads_all_in_order() {
        let bundle = build_bundle(&[b"\x01\x02\x03", b"hello", b"DXBCmagicpayload"]);
        let entries = parse_bundle_entries(&bundle).expect("parse should succeed");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], b"\x01\x02\x03");
        assert_eq!(entries[1], b"hello");
        assert_eq!(entries[2], b"DXBCmagicpayload");
    }

    #[test]
    fn parse_bundle_entry_at_returns_correct_entry() {
        let bundle = build_bundle(&[b"first", b"second-entry", b"third"]);
        assert_eq!(parse_bundle_entry_at(&bundle, 0).unwrap(), b"first");
        assert_eq!(parse_bundle_entry_at(&bundle, 1).unwrap(), b"second-entry");
        assert_eq!(parse_bundle_entry_at(&bundle, 2).unwrap(), b"third");
    }

    /// Regression: `Vec::with_capacity(n) + read_exact(&mut v)` reads zero bytes
    /// because `len() == 0`. Both helpers must use `vec![0u8; n]` instead.
    #[test]
    fn parse_bundle_does_not_return_empty_entries_for_nonempty_input() {
        let payload = b"non-empty-bytecode";
        let bundle = build_bundle(&[payload]);
        let entries = parse_bundle_entries(&bundle).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].len(), payload.len());
        assert_eq!(entries[0], payload);

        let single = parse_bundle_entry_at(&bundle, 0).unwrap();
        assert_eq!(single.len(), payload.len());
        assert_eq!(single, payload);
    }

    #[test]
    fn parse_bundle_entries_empty_input_yields_no_entries() {
        let entries = parse_bundle_entries(&[]).unwrap();
        assert!(entries.is_empty());
    }
}
