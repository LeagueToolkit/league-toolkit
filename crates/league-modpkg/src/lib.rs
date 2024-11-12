use chunk::ModpkgChunk;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod chunk;
mod error;
mod read;

#[derive(Debug, PartialEq)]
pub struct Modpkg {
    metadata: ModpkgMetadata,
    chunk_paths: Vec<String>,
    wad_paths: Vec<String>,
    chunks: HashMap<u64, ModpkgChunk>,
}

impl Modpkg {
    pub fn metadata(&self) -> &ModpkgMetadata {
        &self.metadata
    }
    pub fn chunks(&self) -> &HashMap<u64, ModpkgChunk> {
        &self.chunks
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ModpkgMetadata {
    name: String,
    display_name: String,
    description: Option<String>,
    version: String,
    distributor: Option<String>,
    authors: Vec<ModpkgAuthor>,
    license: ModpkgLicense,
}

impl ModpkgMetadata {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    pub fn version(&self) -> &str {
        &self.version
    }
    pub fn distributor(&self) -> Option<&str> {
        self.distributor.as_deref()
    }
    pub fn authors(&self) -> &[ModpkgAuthor] {
        &self.authors
    }
    pub fn license(&self) -> &ModpkgLicense {
        &self.license
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub enum ModpkgLicense {
    None,
    Spdx { spdx_id: String },
    Custom { name: String, url: String },
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct ModpkgAuthor {
    name: String,
    role: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ModpkgCompression {
    None = 0,
    Zstd = 1,
}

impl TryFrom<u8> for ModpkgCompression {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => ModpkgCompression::None,
            1 => ModpkgCompression::Zstd,
            _ => return Err("Invalid modpkg compression value"),
        })
    }
}
