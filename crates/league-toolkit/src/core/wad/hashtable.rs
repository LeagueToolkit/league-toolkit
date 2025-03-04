use std::{
    collections::HashMap,
    io::{self, BufRead, BufReader},
    num::ParseIntError,
};

#[derive(Debug, thiserror::Error)]
pub enum WadHashtableError {
    #[error("Invalid hash: {0}")]
    InvalidHash(#[from] ParseIntError),

    #[error(transparent)]
    Io(#[from] io::Error),
}

pub type WadHashtable = HashMap<u64, String>;

pub enum WadHashtableHash<'a> {
    Default(&'a str),
    Unknown(String),
}

pub trait WadHashtableExt {
    fn from_reader(reader: impl io::Read) -> Result<WadHashtable, WadHashtableError>;

    fn resolve(&self, hash: u64) -> Option<&str>;
    fn resolve_or_default(&self, hash: u64) -> WadHashtableHash;
}

impl WadHashtableExt for WadHashtable {
    fn from_reader(reader: impl io::Read) -> Result<Self, WadHashtableError> {
        let reader = BufReader::new(reader);
        let mut hashtable = HashMap::new();

        for line in reader.lines() {
            let line = line?;

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let hash = parts[0].trim();
                let path = parts[1].trim();
                hashtable.insert(u64::from_str_radix(hash, 16)?, path.to_string());
            }
        }

        Ok(hashtable)
    }

    fn resolve(&self, hash: u64) -> Option<&str> {
        match self.get(&hash) {
            Some(path) => Some(path.as_str()),
            None => None,
        }
    }

    fn resolve_or_default(&self, hash: u64) -> WadHashtableHash {
        match self.resolve(hash) {
            Some(path) => WadHashtableHash::Default(path),
            None => WadHashtableHash::Unknown(format!("0x{:x}", hash)),
        }
    }
}
