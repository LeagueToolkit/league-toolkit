use crate::defines::ShaderMacroDefinition;
use crate::read_sized_string;
use byteorder::{ReadBytesExt, LE};
use std::io::{self, Read};

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

    pub fn read<R: Read>(reader: &mut R) -> io::Result<Self> {
        let toc_magic = read_sized_string(reader)?;
        if toc_magic != "TOC3.0" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid TOC magic: expected TOC3.0, got {}", toc_magic),
            ));
        }

        let shader_count = reader.read_u32::<LE>()?;
        let base_defines_count = reader.read_u32::<LE>()?;
        let _bundled_shader_count = reader.read_u32::<LE>()?; // unused
        let _shader_type = reader.read_u32::<LE>()?; // 0=vs, 1=ps

        let base_defines_section_magic = read_sized_string(reader)?;
        if base_defines_section_magic != "baseDefines" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid baseDefines section magic",
            ));
        }

        let mut base_defines = Vec::with_capacity(base_defines_count as usize);
        for _ in 0..base_defines_count {
            base_defines.push(ShaderMacroDefinition::read(reader)?);
        }

        let shaders_section_magic = read_sized_string(reader)?;
        if shaders_section_magic != "shaders" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid shaders section magic",
            ));
        }

        let mut shader_hashes = vec![0u64; shader_count as usize];
        let mut shader_ids = vec![0u32; shader_count as usize];

        reader.read_u64_into::<LE>(&mut shader_hashes)?;
        reader.read_u32_into::<LE>(&mut shader_ids)?;

        Ok(Self {
            base_defines,
            shader_hashes,
            shader_ids,
        })
    }
}
