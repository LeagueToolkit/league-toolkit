use crate::ast::{
    node::root::{FileKind, KnownRoot, Root, RootKind, RootValue},
    RootEntry,
};

pub type VersionRoot = KnownRoot<u32>;
pub type FileTypeRoot = KnownRoot<FileKind>;
pub type LinkedRoot = KnownRoot<Vec<String>>;

#[derive(Debug, Clone, Default)]
pub struct Roots {
    pub(crate) file_type: Option<FileTypeRoot>,
    pub(crate) version: Option<VersionRoot>,
    pub(crate) linked: Option<LinkedRoot>,
    pub(crate) entries: Option<usize>,

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
