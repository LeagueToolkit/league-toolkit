//! The table of contents a sweep harvests: one plain-data row per object.

use std::collections::HashMap;

use ltk_hash::BinHash;

/// One row of the table of contents.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectEntry {
    /// The object's path hash.
    pub path_hash: BinHash,
    /// The object's class hash, from the table read at mount.
    pub class_hash: BinHash,
    /// Absolute offset of the object's `u32` size field.
    pub offset: u64,
    /// Declared byte size of the object body (as the file states it).
    pub size: u32,
}

impl ObjectEntry {
    /// The object's raw byte range in the stream, size field included.
    ///
    /// This is the range a byte-exact copy of the object covers.
    #[must_use]
    pub fn byte_range(&self) -> std::ops::Range<u64> {
        self.offset..self.offset + 4 + u64::from(self.size)
    }
}

/// File-order entries plus a hash index.
///
/// Plain data: `Clone`, so a consumer can detach it from the handle that built it, and
/// serializable behind the `serde` feature so it can be persisted (only the entries are
/// serialized; the index is rebuilt on the way in).
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(from = "Vec<ObjectEntry>", into = "Vec<ObjectEntry>")
)]
#[derive(Debug, Clone, Default)]
pub struct BinToc {
    entries: Vec<ObjectEntry>,
    index: HashMap<BinHash, usize>,
}

impl BinToc {
    /// The entries, in file order.
    #[must_use]
    pub fn entries(&self) -> &[ObjectEntry] {
        &self.entries
    }

    /// The entry for the object with the given path hash, if the TOC holds one.
    #[must_use]
    pub fn entry(&self, path_hash: impl Into<BinHash>) -> Option<&ObjectEntry> {
        self.entries.get(*self.index.get(&path_hash.into())?)
    }

    /// Appends one harvested row.
    ///
    /// On the (never shipped) chance two objects share a path hash, the index keeps the
    /// last, matching what the eager reader's map keeps addressable.
    pub(crate) fn push(&mut self, entry: ObjectEntry) {
        self.index.insert(entry.path_hash, self.entries.len());
        self.entries.push(entry);
    }
}

impl From<Vec<ObjectEntry>> for BinToc {
    /// Rebuilds the hash index over `entries`.
    fn from(entries: Vec<ObjectEntry>) -> Self {
        let mut toc = Self::default();
        for entry in entries {
            toc.push(entry);
        }
        toc
    }
}

impl From<BinToc> for Vec<ObjectEntry> {
    fn from(toc: BinToc) -> Self {
        toc.entries
    }
}
