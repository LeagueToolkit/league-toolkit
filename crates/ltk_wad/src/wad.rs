use std::collections::BTreeMap;

use binrw::{binrw, BinRead, BinWrite};
use itertools::Itertools;

use crate::{
    entry::{self, EntryExt},
    header::{Header, HeaderExt},
};

#[binrw]
#[brw(magic = b"RW", little)]
#[derive(Debug, Clone)]
pub struct Wad {
    #[bw(calc = self.major())]
    pub major: u8,
    pub minor: u8,
    #[brw(args(major))]
    pub header: Header,
    #[br(args{major, minor, count: header.entry_count().try_into().unwrap()})]
    #[bw(args{major, minor: *minor, count: header.entry_count().try_into().unwrap()})]
    pub entries: Entries,
}

type EntriesDyn<'a> = Box<dyn Iterator<Item = &'a dyn EntryExt> + 'a>;

impl Wad {
    pub fn major(&self) -> u8 {
        self.header.major()
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }

    pub fn version(&self) -> (u8, u8) {
        (self.major(), self.minor())
    }

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
#[brw(import { major: u8, minor: u8, count: usize })]
#[derive(Debug, Clone)]
pub enum Entries {
    #[br(pre_assert(major == 3 && minor == 4))]
    V3_4(
        #[br(count = count, map = vec_to_btree::<entry::V3_4>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V3_4>,
    ),
    #[br(pre_assert(major == 3 && minor < 4))]
    V3(
        #[br(count = count, map = vec_to_btree::<entry::V3>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V3>,
    ),
    #[br(pre_assert(major == 2))]
    V2(
        #[br(count = count, map = vec_to_btree::<entry::V2>)]
        #[bw(map = |e| e.values().copied().collect_vec())]
        BTreeMap<u64, entry::V2>,
    ),
    #[br(pre_assert(major == 1))]
    V1(
        #[br(count = count, map = vec_to_btree::<entry::V1>)]
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
