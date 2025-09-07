use binrw::BinWrite;
use xxhash_rust::xxh3;

use crate::{
    entry::{self, EntryExt, OwnedEntry, WriteableEntry},
    header::{self, Header, Headers},
    RawWad, VersionArgs, Wad,
};
use std::{
    collections::BTreeMap,
    io::{self, SeekFrom},
};

pub struct Builder<Signature = (), Data = ()> {
    signature: Signature,
    entries: BTreeMap<u64, Data>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            signature: (),
            entries: Default::default(),
        }
    }
}

impl<Data> Builder<(), Data> {
    pub fn from_entries(entries: BTreeMap<u64, OwnedEntry<Data>>) -> Builder<(), OwnedEntry<Data>> {
        Builder {
            entries,
            signature: (),
        }
    }
}

impl<S, Data> Builder<S, Data> {
    pub fn new() -> Builder<()> {
        Builder::default()
    }

    pub fn with_signature(self, signature: [u8; 256]) -> Builder<[u8; 256], Data> {
        Builder {
            entries: self.entries,
            signature,
        }
    }
}

impl<D: WriteableEntry> Builder<[u8; 256], D> {
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

        let toc_pos = writer.stream_position()?;

        let dummy_entry = entry::Latest {
            path_hash: 0,
            data_offset: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            kind: entry::EntryKind::None,
            subchunk_count: 0,
            subchunk_index: 0,
            checksum: 0,
        };

        dummy_entry.write(writer).unwrap();

        let entry_size = (writer.stream_position()? - toc_pos) as i64;
        let toc_size = entry_size * self.entries.len() as i64;

        let data_start = writer.seek(SeekFrom::Current(toc_size - entry_size))?;

        let mut offsets = Vec::with_capacity(self.entries.len());
        let mut total_written = 0;
        // write data
        for (_path, entry) in &self.entries {
            let written = entry.write_data(writer)?;
            offsets.push(data_start as u32 + total_written as u32);
            total_written += written;
        }

        writer.seek_relative(-(total_written as i64 + toc_size))?;

        // write toc
        for ((_path, entry), data_off) in self.entries.iter().zip(offsets) {
            entry.write_entry(writer, data_off)?;
        }

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
