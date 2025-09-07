use crate::entry::{EntryExt, EntryKind};

pub struct Builder<D> {
    pub path_hash: u64,
    pub checksum: Option<u64>,
    pub kind: EntryKind,
    pub data: D,
}

impl<T: EntryExt> From<T> for Builder<()> {
    fn from(value: T) -> Builder<()> {
        Builder {
            path_hash: value.path_hash(),
            checksum: value.checksum(),
            kind: value.kind(),
            data: (),
        }
    }
}

impl<D> Builder<D> {
    pub fn with_checksum(mut self, checksum: u64) -> Self {
        self.checksum.replace(checksum);
        self
    }

    pub fn with_data<NewData>(self, data: NewData) -> Builder<NewData> {
        Builder {
            path_hash: self.path_hash,
            checksum: self.checksum,
            kind: self.kind,
            data,
        }
    }
}
