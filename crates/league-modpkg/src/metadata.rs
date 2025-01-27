use byteorder::{ReadBytesExt as _, LE};

use crate::license::ModpkgLicense;
use crate::utils::length_prefixed_string_size;
use crate::{error::ModpkgError, utils::non_empty_string};
use io_ext::{ReaderExt as _, WriterExt as _};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Read, Write};

#[derive(Debug, PartialEq, Default)]
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
    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let name = reader.read_len_prefixed_string::<LE>()?;
        let display_name = reader.read_len_prefixed_string::<LE>()?;
        let description = non_empty_string(reader.read_len_prefixed_string::<LE>()?);
        let version = reader.read_len_prefixed_string::<LE>()?;
        let distributor = non_empty_string(reader.read_len_prefixed_string::<LE>()?);

        let authors = Self::read_authors(reader)?;
        let license = ModpkgLicense::read(reader)?;

        Ok(Self {
            name,
            display_name,
            description,
            version,
            distributor,
            authors,
            license,
        })
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), ModpkgError> {
        writer.write_len_prefixed_string_better::<LE>(&self.name)?;
        writer.write_len_prefixed_string_better::<LE>(&self.display_name)?;
        writer
            .write_len_prefixed_string_better::<LE>(self.description.as_ref().map_or("", |v| v))?;
        writer.write_len_prefixed_string_better::<LE>(&self.version)?;
        writer
            .write_len_prefixed_string_better::<LE>(self.distributor.as_ref().map_or("", |v| v))?;

        for author in &self.authors {
            author.write(writer)?;
        }

        self.license.write(writer)?;

        Ok(())
    }

    fn read_authors(reader: &mut BufReader<impl Read>) -> Result<Vec<ModpkgAuthor>, ModpkgError> {
        let count = reader.read_u32::<LE>()?;
        let mut authors = Vec::with_capacity(count as usize);
        for _ in 0..count {
            authors.push(ModpkgAuthor::read(reader)?);
        }

        Ok(authors)
    }

    pub fn size(&self) -> usize {
        let mut size = 0;

        size += length_prefixed_string_size(&self.name);
        size += length_prefixed_string_size(&self.display_name);
        size += length_prefixed_string_size(self.description.as_ref().map_or("", |v| v));
        size += length_prefixed_string_size(&self.version);
        size += length_prefixed_string_size(self.distributor.as_ref().map_or("", |v| v));
        size += 4 + self.authors.iter().map(|a| a.size()).sum::<usize>();
        size += self.license.size();

        size
    }
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
pub struct ModpkgAuthor {
    name: String,
    role: Option<String>,
}

impl ModpkgAuthor {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn role(&self) -> Option<&str> {
        self.role.as_deref()
    }
}

impl ModpkgAuthor {
    pub fn read(reader: &mut BufReader<impl Read>) -> Result<Self, ModpkgError> {
        let name = reader.read_len_prefixed_string::<LE>()?;
        let role = non_empty_string(reader.read_len_prefixed_string::<LE>()?);

        Ok(Self { name, role })
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<(), ModpkgError> {
        writer.write_len_prefixed_string_better::<LE>(&self.name)?;
        writer.write_len_prefixed_string_better::<LE>(self.role.as_ref().map_or("", |v| v))?;
        Ok(())
    }

    pub fn size(&self) -> usize {
        length_prefixed_string_size(&self.name)
            + length_prefixed_string_size(self.role.as_ref().map_or("", |v| v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_size() {
        let metadata = ModpkgMetadata::default();

        assert_eq!(metadata.size(), 25);
    }

    #[test]
    fn test_size() {
        let mod_name = "test".to_string();
        let display_name = "test".to_string();
        let description = "test".to_string();
        let version = "test".to_string();
        let distributor = "test".to_string();
        let author_name = "test".to_string();
        let author_role = "test".to_string();
        let license = ModpkgLicense::Spdx {
            spdx_id: "test".to_string(),
        };

        let expected_size = length_prefixed_string_size(&mod_name)
            + length_prefixed_string_size(&display_name)
            + length_prefixed_string_size(&description)
            + length_prefixed_string_size(&version)
            + length_prefixed_string_size(&distributor)
            + 4
            + length_prefixed_string_size(&author_name)
            + length_prefixed_string_size(&author_role)
            + 1
            + 8;

        let metadata = ModpkgMetadata {
            name: mod_name,
            display_name,
            description: Some(description),
            version,
            distributor: Some(distributor),
            authors: vec![ModpkgAuthor {
                name: author_name,
                role: Some(author_role),
            }],
            license,
        };

        assert_eq!(metadata.size(), expected_size);
    }
}
