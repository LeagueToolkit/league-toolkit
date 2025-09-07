use std::{
    io::{self, Read as _},
    ops::Deref,
};

use binrw::{meta::WriteEndian, BinWrite};
use derive_more::DerefMut;
use flate2::read::GzDecoder;
use memchr::memmem;
use xxhash_rust::xxh3;

use crate::{
    entry::{self, WriteableEntry},
    VersionArgs,
};

use super::{Decompress, Entry, EntryExt, EntryKind};

#[derive(Clone, Debug)]
pub struct DataRegion<T> {
    pub data: T,
    pub off: u32,
    pub length: u32,
}

impl<T> AsRef<[u8]> for DataRegion<T>
where
    T: Deref,
    T::Target: AsRef<[u8]>,
{
    fn as_ref(&self) -> &[u8] {
        &self.data.as_ref()[self.off as usize..self.off as usize + self.length as usize]
    }
}

#[derive(derive_more::Deref, DerefMut)]
pub struct OwnedEntry<D, E: EntryExt = Entry> {
    #[deref_mut]
    #[deref]
    inner: E,
    data: DataRegion<D>,
}

impl<D: std::fmt::Debug> std::fmt::Debug for OwnedEntry<D> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedEntry")
            .field("inner", &self.inner)
            .field("data", &self.data)
            .finish()
    }
}

impl<E: EntryExt, D> OwnedEntry<D, E> {
    pub fn new(entry: E, data: D) -> Self {
        Self {
            data: DataRegion {
                data,
                length: entry.compressed_size(),
                off: entry.data_offset(),
            },
            inner: entry,
        }
    }
}

impl<E: EntryExt, D> WriteableEntry for OwnedEntry<D, E>
where
    D: Deref,
    D::Target: AsRef<[u8]>,
    for<'a> E: BinWrite<Args<'a> = VersionArgs> + WriteEndian,
{
    fn write_entry<W: io::Write + io::Seek>(
        &self,
        writer: &mut W,
        data_off: u32,
        checksum: u64,
    ) -> io::Result<()> {
        let mut entry = entry::Latest::from_generic_or_default(&self.inner);
        entry.data_offset = data_off;
        entry.checksum = checksum;
        entry.write(writer).unwrap();
        Ok(())
    }

    fn write_data<W: io::Write>(&self, writer: &mut W) -> io::Result<(usize, u64)> {
        let data = self.data.as_ref();
        let checksum = xxh3::xxh3_64(data);
        let written = writer.write(data)?;
        Ok((written, checksum))
    }
}
const ZSTD_MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];

impl<D> Decompress for OwnedEntry<D>
where
    D: Deref,
    D::Target: AsRef<[u8]>,
{
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
