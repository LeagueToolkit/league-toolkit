//! Generated bin pairs for the merge and diff property tests.
//!
//! Every field name carries a family of kinds, so the same field on two generated objects holds
//! the same shape most of the time and a different one some of the time. Object hashes, classes,
//! keys and leaf values come from small sets, so two bins meet on many objects, nodes and keys.
//! No float is `NaN`: a `NaN` leaf never equals itself.

#![expect(dead_code, reason = "each test binary uses a different subset")]

use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;
use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{
    property::{values, Kind, NoMeta},
    Bin, BinObject, PropertyValueEnum,
};
use proptest::{prelude::*, strategy::Union};

/// Every field name a generated bin uses.
pub const FIELDS: [&str; 10] = [
    "Int", "Float", "Text", "Node", "Embed", "List", "Nodes", "Maybe", "Lookup", "Names",
];

pub const CLASSES: [u32; 2] = [0xc1a5_0001, 0xc1a5_0002];

/// A table naming every field a generated bin uses.
pub fn names() -> HashMap<BinHash, String> {
    FIELDS
        .iter()
        .map(|name| (BinHash::hash_str(name), (*name).to_owned()))
        .collect()
}

fn hash(name: &str) -> BinHash {
    BinHash::hash_str(name)
}

fn int() -> BoxedStrategy<PropertyValueEnum> {
    (0i32..3).prop_map(|v| values::I32::new(v).into()).boxed()
}

fn node(class: u32, properties: IndexMap<BinHash, PropertyValueEnum>) -> values::Struct {
    values::Struct {
        class_hash: class.into(),
        // The null pointer has no properties to write.
        properties: if class == 0 {
            IndexMap::new()
        } else {
            properties
        },
        meta: NoMeta,
    }
}

fn class() -> impl Strategy<Value = u32> {
    prop::sample::select(CLASSES.to_vec())
}

fn class_or_null() -> impl Strategy<Value = u32> {
    prop::sample::select(vec![0, CLASSES[0], CLASSES[1]])
}

/// One property of a node `depth` levels above the deepest.
fn property(depth: u32) -> BoxedStrategy<(BinHash, PropertyValueEnum)> {
    let mut options: Vec<BoxedStrategy<(BinHash, PropertyValueEnum)>> = vec![
        // A different kind at the same field some of the time.
        prop_oneof![
            4 => int(),
            1 => any::<bool>().prop_map(|v| values::Bool::new(v).into()),
        ]
        .prop_map(|v| (hash("Int"), v))
        .boxed(),
        prop::sample::select(vec![0.0f32, -0.0, 1.5])
            .prop_map(|v| (hash("Float"), values::F32::new(v).into()))
            .boxed(),
        // The 16.17 migration shape: a string where the game holds a file.
        prop_oneof![
            prop::sample::select(vec!["a", "b"]).prop_map(|v| values::String::from(v).into()),
            (0u64..2).prop_map(|v| values::WadChunkLink::new(v).into()),
        ]
        .prop_map(|v| (hash("Text"), v))
        .boxed(),
        prop::collection::vec(0i32..3, 0..3)
            .prop_map(|items| {
                let list: values::Container = items.into_iter().map(values::I32::new).collect();
                (hash("List"), list.into())
            })
            .boxed(),
        prop::collection::vec((prop::sample::select(vec!["x", "y", "z"]), 0i32..3), 0..3)
            .prop_map(|entries| {
                let entries = unique(entries, |(key, _)| *key)
                    .into_iter()
                    .map(|(key, value)| {
                        (
                            values::String::from(key).into(),
                            values::I32::new(value).into(),
                        )
                    })
                    .collect();
                let map = values::Map::new(Kind::String, Kind::I32, entries).unwrap();
                (hash("Names"), map.into())
            })
            .boxed(),
    ];

    if depth > 0 {
        let inner = || properties(depth - 1);
        options.extend([
            (class_or_null(), inner())
                .prop_map(|(class, properties)| (hash("Node"), node(class, properties).into()))
                .boxed(),
            (class(), inner())
                .prop_map(|(class, properties)| {
                    (
                        hash("Embed"),
                        values::Embedded(node(class, properties)).into(),
                    )
                })
                .boxed(),
            prop::collection::vec((class_or_null(), inner()), 0..3)
                .prop_map(|items| {
                    let items = items
                        .into_iter()
                        .map(|(class, properties)| node(class, properties).into())
                        .collect();
                    let list = values::Container::new(Kind::Struct, items).unwrap();
                    (hash("Nodes"), list.into())
                })
                .boxed(),
            prop::option::of((class(), inner()))
                .prop_map(|held| {
                    let held = held.map(|(class, properties)| node(class, properties).into());
                    let option = values::Optional::new(Kind::Struct, held).unwrap();
                    (hash("Maybe"), option.into())
                })
                .boxed(),
            prop::collection::vec((0u32..4, class_or_null(), inner()), 0..4)
                .prop_map(|entries| {
                    let entries = unique(entries, |(key, ..)| *key)
                        .into_iter()
                        .map(|(key, class, properties)| {
                            (
                                values::Hash::new(key).into(),
                                node(class, properties).into(),
                            )
                        })
                        .collect();
                    let map = values::Map::new(Kind::Hash, Kind::Struct, entries).unwrap();
                    (hash("Lookup"), map.into())
                })
                .boxed(),
        ]);
    }
    Union::new(options).boxed()
}

/// The properties of a node `depth` levels above the deepest.
fn properties(depth: u32) -> BoxedStrategy<IndexMap<BinHash, PropertyValueEnum>> {
    prop::collection::vec(property(depth), 0..5)
        .prop_map(|pairs| pairs.into_iter().collect())
        .boxed()
}

/// A bin of up to four objects over three path hashes, two levels of nodes deep.
pub fn bin() -> impl Strategy<Value = Bin> {
    let objects = prop::collection::vec((1u32..4, class(), properties(2)), 0..4);
    let dependencies = prop::collection::vec(prop::sample::select(vec!["a", "b", "c"]), 0..3);
    (objects, dependencies).prop_map(|(objects, dependencies)| {
        let objects = objects
            .into_iter()
            .map(|(path, class, properties)| BinObject {
                path_hash: path.into(),
                class_hash: class.into(),
                properties,
            });
        Bin::new(objects, unique(dependencies, |d| *d))
    })
}

/// The first item for every key, in order.
fn unique<T, K: std::hash::Hash + Eq>(items: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    let mut seen = HashSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(key(item)))
        .collect()
}
