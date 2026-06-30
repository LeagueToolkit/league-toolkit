use ltk_hash::{BinHash, Hash};

use crate::read_sized_string;
use std::borrow::Cow;
use std::fmt::{self, Debug};
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShaderMacroHash(pub u32);

impl ShaderMacroHash {
    pub fn new(name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        let name = name.as_ref();
        let value = value.as_ref();
        let s = match value.is_empty() {
            true => Cow::from(name),
            false => Cow::from(format!("{}={}", name, value)),
        };

        Self(*BinHash::hash_str(s))
    }
}

#[derive(Debug, Clone)]
pub struct ShaderMacroDefinition {
    pub name: String,
    pub value: String,
    pub hash: ShaderMacroHash,
}

impl ShaderMacroDefinition {
    pub fn new(name: String, value: String) -> Self {
        let hash = ShaderMacroHash::new(&name, &value);
        Self { name, value, hash }
    }

    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let name = read_sized_string(reader)?;
        let value = read_sized_string(reader)?;
        Ok(Self::new(name, value))
    }
}

impl PartialEq for ShaderMacroDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl Eq for ShaderMacroDefinition {}

impl fmt::Display for ShaderMacroDefinition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.value.is_empty() {
            write!(f, "{}", self.name)
        } else {
            write!(f, "{}={}", self.name, self.value)
        }
    }
}
