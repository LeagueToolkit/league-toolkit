//! `Bin::merge`, `ptch-property-patches.md` section 10: the edit wins at every leaf it reaches, and
//! whatever only the base holds survives.

mod common;

use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{
    path::{MapKey, ValuePath, ValueSegment},
    property::{values, Kind, NoMeta},
    Bin, BinObject, PropertyValueEnum,
};
use proptest::prelude::*;

const OBJECT: u32 = 0x0100_0001;
const CLASS: u32 = 0xc1a5_0001;
const INNER: u32 = 0xc1a5_0002;

fn hash(name: &str) -> BinHash {
    BinHash::hash_str(name)
}

fn object(path: u32, class: u32, properties: Vec<(&str, PropertyValueEnum)>) -> BinObject {
    BinObject::<NoMeta>::builder(path, class)
        .properties(
            properties
                .into_iter()
                .map(|(name, value)| (hash(name), value)),
        )
        .build()
}

fn bin(objects: Vec<BinObject>) -> Bin {
    Bin::builder().objects(objects).build()
}

fn node(class: u32, properties: Vec<(&str, PropertyValueEnum)>) -> values::Struct {
    values::Struct {
        class_hash: class.into(),
        properties: properties
            .into_iter()
            .map(|(name, value)| (hash(name), value))
            .collect(),
        meta: NoMeta,
    }
}

fn int(value: i32) -> PropertyValueEnum {
    values::I32::new(value).into()
}

fn text(value: &str) -> PropertyValueEnum {
    values::String::from(value).into()
}

/// A `Hash -> String` map.
fn lookup(entries: &[(u32, &str)]) -> PropertyValueEnum {
    values::Map::new(
        Kind::Hash,
        Kind::String,
        entries
            .iter()
            .map(|(key, value)| (values::Hash::new(*key).into(), text(value)))
            .collect(),
    )
    .unwrap()
    .into()
}

fn path(segments: impl IntoIterator<Item = ValueSegment>) -> ValuePath {
    segments.into_iter().collect()
}

fn field(name: &str) -> ValueSegment {
    ValueSegment::Field(hash(name))
}

#[test]
fn what_only_the_base_holds_survives_and_new_keys_follow_the_base_in_edit_order() {
    let mut base = bin(vec![object(
        OBJECT,
        CLASS,
        vec![
            ("Kept", int(1)),
            ("Lookup", lookup(&[(1, "a"), (2, "b"), (3, "c")])),
        ],
    )]);
    let edited = bin(vec![object(
        OBJECT,
        CLASS,
        vec![
            ("Lookup", lookup(&[(9, "z"), (2, "B"), (8, "y")])),
            ("Added", int(7)),
        ],
    )]);

    let report = base.merge(&edited);

    let expected = bin(vec![object(
        OBJECT,
        CLASS,
        vec![
            ("Kept", int(1)),
            (
                "Lookup",
                lookup(&[(1, "a"), (2, "B"), (3, "c"), (9, "z"), (8, "y")]),
            ),
            ("Added", int(7)),
        ],
    )]);
    assert_eq!(base, expected);
    let merged = &base.objects[&BinHash(OBJECT)].properties[&hash("Lookup")];
    assert_eq!(
        merged,
        &expected.objects[&BinHash(OBJECT)].properties[&hash("Lookup")]
    );
    let PropertyValueEnum::Map(map) = merged else {
        panic!("Lookup is a map");
    };
    let keys: Vec<_> = map
        .entries()
        .iter()
        .map(|(key, _)| MapKey::try_from(key).unwrap())
        .collect();
    assert_eq!(
        keys,
        [1, 2, 3, 9, 8].map(|key| MapKey::Hash(BinHash(key))),
        "base keys first, then the edit's new keys in its order"
    );

    assert_eq!(report.objects_merged, [BinHash(OBJECT)]);
    assert_eq!(report.inserted, 1);
    assert_eq!(report.keys_inserted, 2);
    assert_eq!(report.replaced.len(), 1);
    let replaced = &report.replaced[0];
    assert_eq!(replaced.object_hash, BinHash(OBJECT));
    assert_eq!(
        replaced.at,
        path([field("Lookup"), ValueSegment::Key(MapKey::Hash(BinHash(2)))])
    );
    assert_eq!(replaced.was, text("b"));
    assert!(!replaced.mismatched);
}

fn one(properties: Vec<(&str, PropertyValueEnum)>) -> Bin {
    bin(vec![object(OBJECT, CLASS, properties)])
}

fn file(value: u64) -> PropertyValueEnum {
    values::WadChunkLink::new(value).into()
}

#[test]
fn a_string_merged_over_a_file_of_the_same_field_is_one_mismatch() {
    // The 16.17 `String` -> `File` migration: the game's copy holds a `File`, a mod that predates
    // the patch still carries a `String`.
    let mut base = one(vec![("texturePath", file(0x1234))]);
    let edited = one(vec![("texturePath", text("ASSETS/old.dds"))]);

    let report = base.merge(&edited);

    assert_eq!(base, edited);
    assert_eq!(report.replaced.len(), 1);
    assert_eq!(report.replaced[0].at, path([field("texturePath")]));
    assert_eq!(report.replaced[0].was, file(0x1234));
    assert!(report.replaced[0].mismatched);
}

#[test]
fn a_node_of_the_same_class_combines_and_one_of_another_class_replaces_whole() {
    let mut base = one(vec![
        (
            "Same",
            node(INNER, vec![("A", int(1)), ("B", int(2))]).into(),
        ),
        ("Other", node(INNER, vec![("A", int(1))]).into()),
        (
            "Embed",
            values::Embedded(node(INNER, vec![("A", int(1))])).into(),
        ),
        ("Null", values::Struct::<NoMeta>::default().into()),
    ]);
    let edited = one(vec![
        ("Same", node(INNER, vec![("B", int(3))]).into()),
        ("Other", node(CLASS, vec![("C", int(1))]).into()),
        (
            "Embed",
            values::Embedded(node(CLASS, vec![("A", int(1))])).into(),
        ),
        ("Null", node(INNER, vec![("A", int(1))]).into()),
    ]);

    let report = base.merge(&edited);

    assert_eq!(
        base,
        one(vec![
            (
                "Same",
                node(INNER, vec![("A", int(1)), ("B", int(3))]).into()
            ),
            ("Other", node(CLASS, vec![("C", int(1))]).into()),
            (
                "Embed",
                values::Embedded(node(CLASS, vec![("A", int(1))])).into()
            ),
            ("Null", node(INNER, vec![("A", int(1))]).into()),
        ])
    );
    let replaced: Vec<_> = report
        .replaced
        .iter()
        .map(|r| (r.at.clone(), r.mismatched))
        .collect();
    assert_eq!(
        replaced,
        [
            (path([field("Same"), field("B")]), false),
            (path([field("Other")]), true),
            (path([field("Embed")]), true),
            (path([field("Null")]), true),
        ]
    );
    let classes: Vec<_> = report.replaced[0].at.fields().collect();
    assert_eq!(
        classes,
        [
            (hash("Same"), Some(BinHash(CLASS))),
            (hash("B"), Some(BinHash(INNER)))
        ]
    );
}

#[test]
fn a_container_replaces_whole_and_an_optional_combines_what_it_holds() {
    let list = |items: &[i32]| -> PropertyValueEnum {
        values::Container::from(
            items
                .iter()
                .map(|i| values::I32::new(*i))
                .collect::<Vec<_>>(),
        )
        .into()
    };
    let option = |value: Option<values::Struct>| -> PropertyValueEnum {
        values::Optional::new(Kind::Struct, value.map(Into::into))
            .unwrap()
            .into()
    };
    let mut base = one(vec![
        ("List", list(&[1, 2, 3])),
        ("Same", list(&[4])),
        ("Held", option(Some(node(INNER, vec![("A", int(1))])))),
        ("Empty", option(None)),
    ]);
    let edited = one(vec![
        ("List", list(&[1, 9])),
        ("Same", list(&[4])),
        ("Held", option(Some(node(INNER, vec![("B", int(2))])))),
        ("Empty", option(Some(node(INNER, vec![])))),
    ]);

    let report = base.merge(&edited);

    assert_eq!(
        base,
        one(vec![
            ("List", list(&[1, 9])),
            ("Same", list(&[4])),
            (
                "Held",
                option(Some(node(INNER, vec![("A", int(1)), ("B", int(2))])))
            ),
            ("Empty", option(Some(node(INNER, vec![])))),
        ])
    );
    let at: Vec<_> = report.replaced.iter().map(|r| r.at.clone()).collect();
    assert_eq!(at, [path([field("List")]), path([field("Empty")])]);
    assert_eq!(report.replaced[0].was, list(&[1, 2, 3]));
    assert_eq!(report.inserted, 1, "`B` inside the optional's struct");
}

#[test]
fn objects_are_added_combined_or_replaced_and_dependencies_merge_as_a_union() {
    let mut base = Bin::builder()
        .dependencies(["a.bin", "b.bin"])
        .objects([
            object(1, CLASS, vec![("A", int(1))]),
            object(2, CLASS, vec![("A", int(1))]),
            object(3, CLASS, vec![("A", int(1))]),
        ])
        .build();
    let edited = Bin::builder()
        .dependencies(["c.bin", "a.bin"])
        .objects([
            object(4, CLASS, vec![("A", int(4))]),
            object(2, CLASS, vec![("B", int(2))]),
            object(3, INNER, vec![("C", int(3))]),
        ])
        .build();

    let report = base.merge(&edited);

    assert_eq!(base.dependencies, ["a.bin", "b.bin", "c.bin"]);
    assert_eq!(
        base,
        Bin::builder()
            .dependencies(["a.bin", "b.bin", "c.bin"])
            .objects([
                object(1, CLASS, vec![("A", int(1))]),
                object(2, CLASS, vec![("A", int(1)), ("B", int(2))]),
                object(3, INNER, vec![("C", int(3))]),
                object(4, CLASS, vec![("A", int(4))]),
            ])
            .build()
    );
    assert_eq!(report.objects_added, [BinHash(4)]);
    assert_eq!(report.objects_merged, [BinHash(2)]);
    assert_eq!(
        report.objects_replaced,
        [object(3, CLASS, vec![("A", int(1))])]
    );
    assert!(report.replaced.is_empty());
    assert_eq!(
        report.to_string(),
        "1 added, 1 merged, 1 replaced; 0 values replaced (0 mismatched), 1 inserted, 0 keys \
         inserted"
    );
}

#[test]
fn a_bin_merged_over_itself_is_unchanged() {
    let original = one(vec![
        ("A", int(1)),
        ("Lookup", lookup(&[(1, "a"), (2, "b")])),
        (
            "Node",
            node(
                INNER,
                vec![(
                    "List",
                    values::Container::from(vec![values::F32::new(-0.0)]).into(),
                )],
            )
            .into(),
        ),
    ]);
    let mut base = original.clone();

    let report = base.merge(&original);

    assert_eq!(base, original);
    assert!(report.is_unchanged(), "{report}");
    assert_eq!(report.objects_merged, [BinHash(OBJECT)]);
}

/// The ADR-0012 specimen of `ltk-manager`, reduced: a mod bin that dropped most of a
/// `ResourceResolver` map, rebound one key and added two.
#[test]
fn a_mod_that_dropped_resolver_keys_gets_them_back_and_keeps_its_own_bindings() {
    const RESOLVER: u32 = 0x0200_0001;
    let game_keys: Vec<(u32, &str)> = (0..20).map(|k| (k, "game")).collect();
    let mut base = bin(vec![object(
        RESOLVER,
        CLASS,
        vec![("resourceMap", lookup(&game_keys))],
    )]);
    let edited = bin(vec![object(
        RESOLVER,
        CLASS,
        vec![(
            "resourceMap",
            lookup(&[(3, "mod"), (100, "mod"), (101, "mod")]),
        )],
    )]);

    let report = base.merge(&edited);

    let PropertyValueEnum::Map(map) =
        &base.objects[&BinHash(RESOLVER)].properties[&hash("resourceMap")]
    else {
        panic!("resourceMap is a map");
    };
    let bindings: Vec<_> = map
        .entries()
        .iter()
        .map(|(key, value)| (MapKey::try_from(key).unwrap(), value.clone()))
        .collect();
    assert_eq!(
        bindings.len(),
        22,
        "every game key survives, both mod keys are added"
    );
    for key in 0..20u32 {
        let value = if key == 3 { "mod" } else { "game" };
        assert!(
            bindings.contains(&(MapKey::Hash(BinHash(key)), text(value))),
            "key {key}"
        );
    }
    for key in [100, 101] {
        assert!(bindings.contains(&(MapKey::Hash(BinHash(key)), text("mod"))));
    }
    assert_eq!(report.keys_inserted, 2);
    assert_eq!(report.replaced.len(), 1);
}

proptest! {
    #[test]
    fn merging_an_edit_twice_changes_nothing_the_first_merge_did_not(
        base in common::bin(),
        edited in common::bin(),
    ) {
        let mut once = base.clone();
        let first = once.merge(&edited);
        let mut twice = once.clone();
        let report = twice.merge(&edited);

        prop_assert_eq!(&twice, &once);
        prop_assert!(report.is_unchanged(), "{}", report);

        // Every replacement is reported where it happened, with what the base held there.
        let names = common::names();
        for replaced in &first.replaced {
            let path = replaced.at.to_property_path(&names).unwrap();
            prop_assert_eq!(base.resolve(replaced.object_hash, &path).unwrap(), &replaced.was);
            prop_assert_eq!(
                once.resolve(replaced.object_hash, &path).unwrap(),
                edited.resolve(replaced.object_hash, &path).unwrap()
            );
        }
    }

    #[test]
    fn a_bin_merged_over_itself_changes_nothing(base in common::bin()) {
        let mut merged = base.clone();
        let report = merged.merge(&base);

        prop_assert_eq!(&merged, &base);
        prop_assert!(report.is_unchanged(), "{}", report);
    }
}
