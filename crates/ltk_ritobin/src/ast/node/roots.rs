use crate::ast::{
    node::root::{FileKind, KnownRoot, Root, RootEntryKind},
    RootEntry,
};

pub type VersionRoot = KnownRoot<u32>;
pub type FileTypeRoot = KnownRoot<FileKind>;
pub type LinkedRoot = KnownRoot<Vec<String>>;
pub type EntriesRoot = KnownRoot<Vec<RootEntry>>;

#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub file_type: Option<FileTypeRoot>,
    pub version: Option<VersionRoot>,
    pub linked: Option<LinkedRoot>,
    pub entries: Option<EntriesRoot>,

    /// Ordered list of all top level roots
    pub all: Vec<Root>,
}

impl Roots {
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

    pub fn contains(&self, kind: RootEntryKind) -> bool {
        match kind {
            RootEntryKind::Unknown => false,
            RootEntryKind::Version => self.version.is_some(),
            RootEntryKind::Type => self.file_type.is_some(),
            RootEntryKind::Linked => self.linked.is_some(),
            RootEntryKind::Entries => self.entries.is_some(),
        }
    }

    pub fn missing(&self) -> impl Iterator<Item = RootEntryKind> + use<'_> {
        [
            RootEntryKind::Version,
            RootEntryKind::Type,
            RootEntryKind::Linked,
            RootEntryKind::Entries,
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
