use ltk_hash::BinHash;

use crate::ast::{
    node::root::{FileKind, KnownRoot, Root, RootKind, RootValue},
    RootEntry, RootPatch,
};

pub type VersionRoot = KnownRoot<u32>;
pub type FileTypeRoot = KnownRoot<FileKind>;
pub type LinkedRoot = KnownRoot<Vec<String>>;
pub type PatchesRoot = KnownRoot<Vec<RootPatch>>;
pub type DeletedRoot = KnownRoot<Vec<BinHash>>;

#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub(crate) file_type: Option<FileTypeRoot>,
    pub(crate) version: Option<VersionRoot>,
    pub(crate) linked: Option<LinkedRoot>,
    pub(crate) entries: Option<usize>,
    pub(crate) patches: Option<PatchesRoot>,
    pub(crate) deleted: Option<DeletedRoot>,

    /// Ordered list of all top level roots
    pub all: Vec<Root>,
}

impl Roots {
    pub fn file_type(&self) -> Option<KnownRoot<FileKind>> {
        self.file_type
    }

    pub fn version(&self) -> Option<KnownRoot<u32>> {
        self.version
    }

    pub fn linked(&self) -> Option<&KnownRoot<Vec<String>>> {
        self.linked.as_ref()
    }

    /// The well-formed records of the `patches` root, in the order they are written.
    ///
    /// A record without a usable path or value is absent here and diagnosed where it is written.
    /// A file that is not `PTCH` has no resolved records.
    pub fn patches(&self) -> Option<&KnownRoot<Vec<RootPatch>>> {
        self.patches.as_ref()
    }

    /// The object hashes of the `deleted` root.
    pub fn deleted(&self) -> Option<&KnownRoot<Vec<BinHash>>> {
        self.deleted.as_ref()
    }

    pub fn new(roots: impl IntoIterator<Item = Root>) -> Self {
        Self {
            all: roots.into_iter().collect(),
            ..Default::default()
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &Root> {
        self.all.iter()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Root> {
        self.all.iter_mut()
    }

    /// The resolved entries of the `entries` root, if present and well-formed.
    pub fn entries(&self) -> Option<&[RootEntry]> {
        match &self.all[self.entries?].value {
            Some(RootValue::Entries(e)) => Some(e.as_slice()),
            _ => None,
        }
    }

    pub fn contains(&self, kind: RootKind) -> bool {
        match kind {
            RootKind::Unknown => false,
            RootKind::Version => self.version.is_some(),
            RootKind::Type => self.file_type.is_some(),
            RootKind::Linked => self.linked.is_some(),
            RootKind::Entries => self.entries.is_some(),
            RootKind::Patches => self.patches.is_some(),
            RootKind::Deleted => self.deleted.is_some(),
        }
    }

    pub fn missing(&self) -> impl Iterator<Item = RootKind> + use<'_> {
        [
            RootKind::Version,
            RootKind::Type,
            RootKind::Linked,
            RootKind::Entries,
        ]
        .into_iter()
        .filter(|k| !self.contains(*k))
    }
}

impl<'a> IntoIterator for &'a Roots {
    type Item = &'a Root;

    type IntoIter = core::slice::Iter<'a, Root>;

    fn into_iter(self) -> Self::IntoIter {
        self.all.iter()
    }
}
