//! Other utilities (hashing, etc)

pub mod elf;
mod fnv1a;
use xxhash_rust::xxh64::xxh64;

pub trait Hash: std::hash::Hash + Eq + Ord + Copy {
    fn from_str(src: impl AsRef<str>) -> Self;
}

/// Wad path hashes - case insensitive
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct WadHash(pub u64);

impl Hash for WadHash {
    fn from_str(src: impl AsRef<str>) -> Self {
        Self(xxh64(src.as_ref().to_ascii_lowercase().as_bytes(), 0))
    }
}

/// Used for bin field/class/property names, etc. - case insensitive
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, std::hash::Hash)]
pub struct BinHash(pub u32);

impl Hash for BinHash {
    fn from_str(src: impl AsRef<str>) -> Self {
        Self(fnv1a::hash_lower(src.as_ref()))
    }
}

pub struct FatHash<H: Hash, V: AsRef<str>> {
    pub hash: H,
    pub value: Option<V>,
}

impl<H: Hash, V: AsRef<str>> From<H> for FatHash<H, V> {
    fn from(value: H) -> Self {
        Self {
            hash: value,
            value: None,
        }
    }
}

impl<H: Hash, V: AsRef<str>> FatHash<H, V> {
    pub fn new(source: V) -> Self {
        Self {
            hash: H::from_str(&source),
            value: Some(source),
        }
    }
}

impl<H: Hash, V: AsRef<str>> std::hash::Hash for FatHash<H, V> {
    fn hash<H2: std::hash::Hasher>(&self, state: &mut H2) {
        self.hash.hash(state)
    }
}
impl<H: Hash, V: AsRef<str>> PartialOrd for FatHash<H, V> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<H: Hash, V: AsRef<str>> Ord for FatHash<H, V> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.hash.cmp(&other.hash)
    }
}

impl<H: Hash, V: AsRef<str>> PartialEq for FatHash<H, V> {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}
impl<H: Hash, V: AsRef<str>> Eq for FatHash<H, V> {}
