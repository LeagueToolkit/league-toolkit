use crate::defines::ShaderMacroDefinition;
use crate::error::{Result, ShaderError};
use crate::read_sized_string;
use byteorder::{ReadBytesExt, LE};
use std::io::Read;

#[derive(Debug)]
pub struct ShaderToc {
    pub base_defines: Vec<ShaderMacroDefinition>,
    pub shader_hashes: Vec<u64>,
    pub shader_ids: Vec<u32>,
}

impl ShaderToc {
    pub fn new(
        base_defines: Vec<ShaderMacroDefinition>,
        shader_hashes: Vec<u64>,
        shader_ids: Vec<u32>,
    ) -> Self {
        Self {
            base_defines,
            shader_hashes,
            shader_ids,
        }
    }

    pub fn read<R: Read>(reader: &mut R) -> Result<Self> {
        let toc_magic = read_sized_string(reader)?;
        if toc_magic != "TOC3.0" {
            return Err(ShaderError::InvalidTocMagic {
                expected: "TOC3.0".to_string(),
                actual: toc_magic,
            });
        }

        let shader_count = reader.read_u32::<LE>()? as usize;
        let base_defines_count = reader.read_u32::<LE>()? as usize;
        let _bundled_shader_count = reader.read_u32::<LE>()?; // unused
        let _shader_type = reader.read_u32::<LE>()?; // 0=vs, 1=ps

        let base_defines_section_magic = read_sized_string(reader)?;
        if base_defines_section_magic != "baseDefines" {
            return Err(ShaderError::InvalidSectionMagic {
                expected: "baseDefines".to_string(),
                actual: base_defines_section_magic,
            });
        }

        let mut base_defines = Vec::with_capacity(base_defines_count);
        for _ in 0..base_defines_count {
            base_defines.push(ShaderMacroDefinition::read(reader)?);
        }

        let shaders_section_magic = read_sized_string(reader)?;
        if shaders_section_magic != "shaders" {
            return Err(ShaderError::InvalidSectionMagic {
                expected: "shaders".to_string(),
                actual: shaders_section_magic,
            });
        }

        let mut shader_hashes = vec![0u64; shader_count];
        let mut shader_ids = vec![0u32; shader_count];

        reader.read_u64_into::<LE>(&mut shader_hashes)?;
        reader.read_u32_into::<LE>(&mut shader_ids)?;

        if shader_hashes.len() != shader_count || shader_ids.len() != shader_count {
            return Err(ShaderError::TocLengthMismatch {
                expected: shader_count,
                hashes: shader_hashes.len(),
                ids: shader_ids.len(),
            });
        }

        Ok(Self {
            base_defines,
            shader_hashes,
            shader_ids,
        })
    }
}
