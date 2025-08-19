use std::collections::BTreeMap;

use binrw::{binrw, BinRead, BinWrite, NamedArgs};
use derive_more::{AsMut, AsRef, Debug, Deref, DerefMut, From, Index, Into};
use itertools::Itertools;

use crate::{
    entry::{self, EntryExt},
    header::{Header, HeaderExt},
};

#[binrw]
#[brw(magic = b"RW", little)]
#[derive(Debug, Clone)]
pub struct Wad<E = EntriesMap>
where
    for<'a> E: BinRead<Args<'a> = EntriesArgs> + BinWrite<Args<'a> = EntriesArgs>,
{
    pub header: Header,
    #[brw(args{major: header.major(), minor: header.minor(), count: header.entry_count().try_into().unwrap()})]
    pub entries: E,
}

#[binrw]
#[brw(import_raw(args: EntriesArgs))]
#[derive(AsMut, AsRef, Debug, Deref, DerefMut, Index, Into, From)]
pub struct EntriesMap(
    #[br(count = args.count)]
    #[br(args {inner: binrw::args!{major: args.major, minor: args.minor}})]
    #[bw(args {major: args.major, minor: args.minor})]
    #[bw(map = |entries| entries.values().copied().collect::<Vec<entry::Entry>>())]
    #[br(map = |entries: Vec<entry::Entry>| entries.into_iter().map(|e| (e.path_hash(), e)).collect() ) ]
    pub BTreeMap<u64, entry::Entry>,
);

#[derive(Clone, NamedArgs)]
pub struct EntriesArgs {
    pub major: u8,
    pub minor: u8,
    pub count: usize,
}

type EntriesDyn<'a> = Box<dyn Iterator<Item = &'a dyn EntryExt> + 'a>;

impl<E> Wad<E>
where
    for<'a> E: BinRead<Args<'a> = EntriesArgs> + BinWrite<Args<'a> = EntriesArgs>,
{
    pub fn major(&self) -> u8 {
        self.header.major()
    }

    pub fn minor(&self) -> u8 {
        self.header.minor()
    }

    pub fn version(&self) -> (u8, u8) {
        (self.major(), self.minor())
    }
}

impl Wad<Entries> {
    pub fn entries_dyn(&self) -> EntriesDyn<'_> {
        match &self.entries {
            Entries::V3_4(btree_map) => Box::new(btree_map.values().map(|v| v as &dyn EntryExt)),
            Entries::V3(btree_map) => Box::new(btree_map.values().map(|v| v as &dyn EntryExt)),
            Entries::V2(btree_map) => Box::new(btree_map.values().map(|v| v as &dyn EntryExt)),
            Entries::V1(btree_map) => Box::new(btree_map.values().map(|v| v as &dyn EntryExt)),
        }
    }
}

#[derive(BinRead, BinWrite)]
#[brw(import_raw(args: EntriesArgs))]
#[derive(Clone, Debug)]
pub enum Entries {
    #[br(pre_assert(args.major == 3 && args.minor == 4))]
    V3_4(
        #[br(count = args.count, map = vec_to_btree::<entry::V3_4>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V3_4>,
    ),
    #[br(pre_assert(args.major == 3 && args.minor < 4))]
    V3(
        #[br(count = args.count, map = vec_to_btree::<entry::V3>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V3>,
    ),
    #[br(pre_assert(args.major == 2))]
    V2(
        #[br(count = args.count, map = vec_to_btree::<entry::V2>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V2>,
    ),
    #[br(pre_assert(args.major == 1))]
    V1(
        #[br(count = args.count, map = vec_to_btree::<entry::V1>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V1>,
    ),
}

fn vec_to_btree<T: EntryExt>(entries: Vec<T>) -> BTreeMap<u64, T> {
    entries
        .into_iter()
        .map(|entry| (entry.path_hash(), entry))
        .collect()
}
