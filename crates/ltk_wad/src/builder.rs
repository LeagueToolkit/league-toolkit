use binrw::BinWrite;
use xxhash_rust::xxh3;

use crate::{
    entry::{self, OwnedEntry},
    header::{self, Header, Headers},
    RawWad, Wad,
};
use std::{collections::BTreeMap, io};

pub struct Builder<Signature = ()> {
    signature: Signature,
    entries: BTreeMap<u64, OwnedEntry<entry::Latest>>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            signature: (),
            entries: Default::default(),
        }
    }
}

impl<S> Builder<S> {
    pub fn new() -> Builder<()> {
        Builder::default()
    }

    pub fn from_entries(entries: BTreeMap<u64, OwnedEntry<entry::Latest>>) -> Builder<()> {
        Builder {
            entries,
            ..Default::default()
        }
    }
}

impl Builder<[u8; 256]> {
    pub fn write_to<W: io::Write + io::Seek>(self, writer: &mut W) -> io::Result<()> {
        let header = Header::new(header::Latest {
            checksum: 0,
            signature: self.signature,
            entry_count: self.entries.len() as u32,
        });

        let start = writer.stream_position()?;
        header.write(writer).expect("TODO: wrap this");

        let mut checksum = xxh3::Xxh3Default::new();
        checksum.update(&[0x52, 0x57, header.major(), header.minor()]);

        // write data
        //
        let header = Header::new(header::Latest {
            checksum: checksum.digest(),
            signature: self.signature,
            entry_count: self.entries.len() as u32,
        });
        writer.seek(io::SeekFrom::Start(start))?;
        header.write(writer).expect("TODO: wrap this");

        Ok(())
    }
}
