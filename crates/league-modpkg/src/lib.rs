use chunk::ModpkgChunk;
use license::ModpkgLicense;
use std::collections::HashMap;

mod chunk;
mod error;
mod license;
mod read;

#[derive(Debug, PartialEq)]
pub struct Modpkg {
    name: String,
    display_name: String,
    description: Option<String>,
    version: String,
    distributor: Option<String>,
    authors: Vec<ModpkgAuthor>,
    license: ModpkgLicense,

    chunks: HashMap<u64, ModpkgChunk>,
}

impl Modpkg {
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
    pub fn chunks(&self) -> &HashMap<u64, ModpkgChunk> {
        &self.chunks
    }
}

#[derive(Debug, PartialEq)]
pub struct ModpkgAuthor {
    name: String,
    role: Option<String>,
}

#[derive(Debug, PartialEq)]
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
