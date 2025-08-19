use std::io::{self, Read as _};

use derive_more::{Deref, DerefMut};
use flate2::read::GzDecoder;
use memchr::memmem;

use super::{Decompress, Entry, EntryExt, EntryKind};

#[derive(Deref, DerefMut)]
pub struct OwnedEntry<D> {
    #[deref_mut]
    #[deref]
    inner: Entry,
    data: D,
}

impl<D> OwnedEntry<D> {
    pub fn new(entry: Entry, data: D) -> Self {
        Self { inner: entry, data }
    }
}

impl<D: AsRef<[u8]>> OwnedEntry<D> {
    pub fn raw_data(&self) -> &[u8] {
        self.data.as_ref()
    }
}
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

impl<D: AsRef<[u8]>> Decompress for OwnedEntry<D> {
    fn decompress(&self) -> io::Result<Vec<u8>> {
        let mut data = self.data.as_ref();
        Ok(match self.kind() {
            EntryKind::None | EntryKind::Satellite => data.to_vec(),
            EntryKind::GZip => {
                let mut out = vec![0; self.uncompressed_size() as _];
                GzDecoder::new(data).read_exact(&mut out)?;
                out
            }
            EntryKind::Zstd => {
                let mut out = vec![0; self.uncompressed_size() as _];
                zstd::Decoder::new(data)
                    .expect("failed to create zstd decoder")
                    .read_exact(&mut out)?;
                out
            }
            EntryKind::ZstdMulti => {
                let mut out = vec![0; self.uncompressed_size() as _];
                let mut search = vec![0; self.compressed_size() as _];
                data.read_exact(&mut search)?;
                let magic_off =
                    memmem::find(&search, &ZSTD_MAGIC).expect("could not find zstd magic");
                for (i, val) in search[..magic_off].iter().enumerate() {
                    out[i] = *val;
                }

                zstd::Decoder::new(&mut &search[magic_off..])
                    .expect("failed to create zstd decoder")
                    .read_exact(&mut out[magic_off..])?;
                out
            }
        })
    }
}

// impl<D: io::Read + io::Seek> OwnedEntry<D> {
//     pub fn decompress(&mut self) -> io::Result<Vec<u8>> {
//         self.kind.decode(
//             &mut self.data,
//             self.compressed_size as _,
//             self.uncompressed_size as _,
//         )
//     }
// }
