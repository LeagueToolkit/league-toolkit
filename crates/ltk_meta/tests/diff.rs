//! `Bin::diff`, `ptch-property-patches.md` section 12: the difference between two bins as a patch,
//! and every place a record could not carry it.

mod common;

use std::{collections::HashMap, io::Cursor};

use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{
    path::{PropertyPath, Subscript, ValuePath, ValueSegment},
    property::{values, NoMeta},
    Bin, BinObject, BinOverride, PropertyPatch, PropertyValueEnum,
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

fn one(properties: Vec<(&str, PropertyValueEnum)>) -> Bin {
    Bin::builder()
        .object(object(OBJECT, CLASS, properties))
        .build()
}

fn node(class: u32, properties: Vec<(&str, PropertyValueEnum)>) -> PropertyValueEnum {
    values::Struct {
        class_hash: class.into(),
        properties: properties
            .into_iter()
            .map(|(name, value)| (hash(name), value))
            .collect(),
        meta: NoMeta,
    }
    .into()
}

fn int(value: i32) -> PropertyValueEnum {
    values::I32::new(value).into()
}

fn names(list: &[&str]) -> HashMap<BinHash, String> {
    list.iter()
        .map(|name| (hash(name), (*name).to_owned()))
        .collect()
}

fn record(path: &str, value: PropertyValueEnum) -> PropertyPatch {
    PropertyPatch::new(OBJECT, PropertyPath::new(path).unwrap(), value)
}

fn at(segments: &[&str]) -> ValuePath {
    segments
        .iter()
        .map(|name| ValueSegment::Field(hash(name)))
        .collect()
}

/// `base.diff(edited)` applied to `base` equals `base.merge(edited)`, and applies cleanly.
fn assert_applies_as_merge(base: &Bin, edited: &Bin, patch: ltk_meta::BinOverride) {
    let mut applied = base.clone();
    let report = patch.apply(&mut applied);
    assert!(report.is_clean(), "{report}: {:?}", report.skipped);
    let mut merged = base.clone();
    merged.merge(edited);
    assert_eq!(applied.objects, merged.objects);
}

#[test]
fn a_changed_leaf_and_a_new_property_are_one_record_each() {
    let base = one(vec![
        ("Same", int(1)),
        ("Changed", int(2)),
        (
            "Node",
            node(INNER, vec![("Deep", int(3)), ("Kept", int(4))]),
        ),
    ]);
    let edited = one(vec![
        ("Changed", int(5)),
        ("Node", node(INNER, vec![("Deep", int(6))])),
        ("Added", int(7)),
    ]);
    let names = names(&["Same", "Changed", "Node", "Deep", "Kept", "Added"]);

    let (patch, report) = base.diff(&edited, &names);

    assert_eq!(
        patch.patches,
        [
            record("Changed", int(5)),
            record("Node.Deep", int(6)),
            record("Added", int(7)),
        ]
    );
    assert!(patch.objects.is_empty() && patch.deleted.is_empty());
    assert_eq!(report.records, 3);
    assert!(report.lifted.is_empty());
    assert_applies_as_merge(&base, &edited, patch);
}

fn lookup(entries: &[(u32, i32)]) -> PropertyValueEnum {
    values::Map::new(
        ltk_meta::property::Kind::Hash,
        ltk_meta::property::Kind::I32,
        entries
            .iter()
            .map(|(key, value)| (values::Hash::new(*key).into(), int(*value)))
            .collect(),
    )
    .unwrap()
    .into()
}

#[test]
fn a_changed_entry_is_a_record_at_its_key_and_a_new_one_lifts_the_map_whole() {
    let base = one(vec![
        ("Changed", lookup(&[(1, 1), (2, 2)])),
        ("Grown", lookup(&[(1, 1), (2, 2)])),
    ]);
    let edited = one(vec![
        ("Changed", lookup(&[(2, 20)])),
        ("Grown", lookup(&[(2, 20), (3, 3), (4, 4)])),
    ]);
    let names = names(&["Changed", "Grown"]);

    let (patch, report) = base.diff(&edited, &names);

    assert_eq!(
        patch.patches,
        [
            record("Changed{2}", int(20)),
            // The base's key 1 is carried: applied to this base, the record loses nothing.
            record("Grown", lookup(&[(1, 1), (2, 20), (3, 3), (4, 4)])),
        ]
    );
    assert_eq!(
        report.lifted,
        [ltk_meta::Lift::MapInsert {
            object_hash: BinHash(OBJECT),
            at: at(&["Grown"]),
            keys: 2,
        }]
    );
    assert_applies_as_merge(&base, &edited, patch);
}

#[test]
fn an_unnamed_field_lifts_to_the_nearest_named_ancestor_or_the_object() {
    let base = one(vec![
        (
            "Node",
            node(INNER, vec![("Secret", int(1)), ("Kept", int(2))]),
        ),
        ("Other", int(3)),
    ]);
    let nested = one(vec![
        ("Node", node(INNER, vec![("Secret", int(9))])),
        ("Other", int(3)),
    ]);

    let (patch, report) = base.diff(&nested, &names(&["Node", "Kept", "Other"]));
    assert_eq!(
        patch.patches,
        [record(
            "Node",
            node(INNER, vec![("Secret", int(9)), ("Kept", int(2))])
        )]
    );
    assert_eq!(report.lifted.len(), 1);
    let ltk_meta::Lift::Nameless {
        at: lifted, cause, ..
    } = &report.lifted[0]
    else {
        panic!("{:?}", report.lifted);
    };
    assert_eq!(lifted, &at(&["Node", "Secret"]));
    assert_eq!(cause.segment, 1);
    assert_applies_as_merge(&base, &nested, patch);

    // With nothing named, the only carrier is the object itself.
    let (patch, report) = base.diff(&nested, &());
    assert!(patch.patches.is_empty());
    assert_eq!(report.objects, [BinHash(OBJECT)]);
    let mut merged = base.objects[&BinHash(OBJECT)].clone();
    merged.merge(&nested.objects[&BinHash(OBJECT)]);
    assert_eq!(patch.objects[&BinHash(OBJECT)], merged);
    assert_eq!(report.lifted.len(), 1);
    assert_applies_as_merge(&base, &nested, patch);
}

#[test]
fn a_changed_shape_lifts_to_the_parent_and_a_changed_class_takes_the_object() {
    let base = Bin::builder()
        .objects([
            object(
                OBJECT,
                CLASS,
                vec![
                    (
                        "Node",
                        node(INNER, vec![("Kind", int(1)), ("Kept", int(2))]),
                    ),
                    ("Pointer", node(INNER, vec![("A", int(1))])),
                    ("Top", int(1)),
                ],
            ),
            object(2, CLASS, vec![("Top", int(1))]),
            object(3, CLASS, vec![("Top", int(1))]),
        ])
        .build();
    let edited = Bin::builder()
        .objects([
            object(
                OBJECT,
                CLASS,
                vec![
                    (
                        "Node",
                        node(INNER, vec![("Kind", values::F32::new(1.0).into())]),
                    ),
                    // A pointer's class is not part of the type rule: this is a plain record.
                    ("Pointer", node(CLASS, vec![("B", int(2))])),
                ],
            ),
            object(
                2,
                CLASS,
                vec![("Top", values::String::from("now text").into())],
            ),
            object(3, INNER, vec![("Top", int(1))]),
        ])
        .build();
    let names = names(&["Node", "Kind", "Kept", "Pointer", "A", "B", "Top"]);

    let (patch, report) = base.diff(&edited, &names);

    assert_eq!(
        patch.patches,
        [
            record(
                "Node",
                node(
                    INNER,
                    vec![("Kind", values::F32::new(1.0).into()), ("Kept", int(2))]
                )
            ),
            record("Pointer", node(CLASS, vec![("B", int(2))])),
        ]
    );
    assert_eq!(report.objects, [BinHash(2), BinHash(3)]);
    let lifted: Vec<_> = report
        .lifted
        .iter()
        .map(|lift| (lift.object_hash(), lift.at().clone()))
        .collect();
    assert_eq!(
        lifted,
        [
            (BinHash(OBJECT), at(&["Node", "Kind"])),
            (BinHash(2), at(&["Top"])),
            (BinHash(3), ValuePath::new()),
        ]
    );
    assert!(report
        .lifted
        .iter()
        .all(|lift| matches!(lift, ltk_meta::Lift::Mismatch { .. })));
    assert_applies_as_merge(&base, &edited, patch);
}

#[test]
fn a_new_object_is_taken_whole_and_an_omitted_one_is_deleted_only_when_asked() {
    let base = Bin::builder()
        .dependencies(["a.bin"])
        .objects([object(1, CLASS, vec![]), object(2, CLASS, vec![])])
        .build();
    let edited = Bin::builder()
        .dependencies(["a.bin", "b.bin"])
        .objects([
            object(1, CLASS, vec![]),
            object(3, CLASS, vec![("A", int(1))]),
        ])
        .build();

    let (patch, report) = base.diff(&edited, &());
    assert_eq!(report.objects, [BinHash(3)]);
    assert!(patch.deleted.is_empty() && report.deleted.is_empty());
    assert_eq!(report.dependencies, ["b.bin"]);
    assert_eq!(
        report.to_string(),
        "0 records, 1 objects, 0 deleted, 1 dependencies, 0 lifted"
    );
    assert_applies_as_merge(&base, &edited, patch);

    let mut options = ltk_meta::DiffOptions::default();
    options.deletions = true;
    let (patch, report) = base.diff_with(&edited, &(), &options);
    assert_eq!(patch.deleted, [BinHash(2)]);
    assert_eq!(report.deleted, [BinHash(2)]);
    let mut applied = base.clone();
    patch.apply(&mut applied);
    assert_eq!(applied.objects, edited.objects);
}

#[test]
fn equal_bins_diff_to_nothing() {
    let base = one(vec![("A", int(1)), ("Map", lookup(&[(1, 1)]))]);
    let (patch, report) = base.diff(&base.clone(), &());
    assert!(patch.is_empty());
    assert_eq!(report, ltk_meta::DiffReport::default());
}

/// A flipped minimap patch and the bin it patches, both from `UI.wad.client` of client
/// 16.16.804.9184.
const UIFLIPPED: &[u8] = include_bytes!("bins/lolminimap_uiflipped.ptch.bin");
const UIBASE: &[u8] = include_bytes!("bins/lolminimap_uibase.bin");

/// A path's segments as `(name hash, subscript)`, which is what the client resolves by.
fn resolved_by(path: &PropertyPath) -> Vec<(BinHash, Option<Subscript<'_>>)> {
    path.segments()
        .map(|segment| (segment.name_hash(), segment.subscript.clone()))
        .collect()
}

#[test]
fn a_shipped_patch_diffs_back_to_records_inside_the_ones_it_carries() {
    let base = Bin::from_reader(&mut Cursor::new(UIBASE)).unwrap();
    let shipped = BinOverride::from_reader(&mut Cursor::new(UIFLIPPED)).unwrap();
    let mut edited = base.clone();
    assert!(shipped.clone().apply(&mut edited).is_clean());
    let mut names: HashMap<BinHash, String> = shipped
        .patches
        .iter()
        .flat_map(|record| record.path.segments())
        .map(|segment| (segment.name_hash(), segment.name.to_owned()))
        .collect();

    // Two shipped records carry a whole `Position.UIRect`, and only its `Size` changes. No
    // record path spells `Size`, so the difference lifts to the embed that holds it.
    let (patch, report) = base.diff(&edited, &names);
    assert_eq!(report.lifted.len(), 2);
    for lift in &report.lifted {
        let ltk_meta::Lift::Nameless {
            at: lifted, cause, ..
        } = lift
        else {
            panic!("{lift}");
        };
        assert_eq!(lifted, &at(&["Position", "UIRect", "Size"]));
        assert_eq!(cause.segment, 2);
    }
    assert_diff_stays_inside(&base, &edited, &shipped, patch);

    // `UiElementRect`'s fields, from the ritobin text of `ptch-property-patches.md` section 15.
    for name in [
        "Position",
        "Size",
        "SourceResolutionWidth",
        "SourceResolutionHeight",
    ] {
        names.insert(hash(name), name.to_owned());
    }
    let (patch, report) = base.diff(&edited, &names);
    assert!(report.lifted.is_empty(), "{:?}", report.lifted);
    assert_diff_stays_inside(&base, &edited, &shipped, patch);
}

/// Every record of `patch` lies inside a record of `shipped` on the same object, and `patch`
/// applied to `base` is `edited`.
fn assert_diff_stays_inside(base: &Bin, edited: &Bin, shipped: &BinOverride, patch: BinOverride) {
    assert!(patch.objects.is_empty());
    assert!(!patch.patches.is_empty());
    for record in &patch.patches {
        let path = resolved_by(&record.path);
        assert!(
            shipped.patches.iter().any(|original| {
                original.object_hash == record.object_hash
                    && path.starts_with(&resolved_by(&original.path))
            }),
            "{:08x} {} is outside every shipped record",
            record.object_hash,
            record.path
        );
    }
    let mut applied = base.clone();
    assert!(patch.apply(&mut applied).is_clean());
    assert_eq!(&applied, edited);
}

/// A name table holding the generated fields `keep` selects.
fn some_names(keep: &[bool]) -> HashMap<BinHash, String> {
    common::FIELDS
        .iter()
        .zip(keep.iter().cycle())
        .filter(|(_, keep)| **keep)
        .map(|(name, _)| (hash(name), (*name).to_owned()))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The invariant of section 12: whatever was lifted, the patch applied to the base it was
    /// made from is the merge.
    #[test]
    fn a_diff_applied_to_its_base_is_the_merge(
        base in common::bin(),
        edited in common::bin(),
        keep in prop::collection::vec(any::<bool>(), common::FIELDS.len()),
    ) {
        for names in [common::names(), some_names(&keep), HashMap::new()] {
            let (patch, report) = base.diff(&edited, &names);
            prop_assert_eq!(report.records, patch.patches.len());
            prop_assert!(patch.check(&base).is_clean());

            let mut applied = base.clone();
            let applied_report = patch.apply(&mut applied);
            prop_assert!(applied_report.is_clean(), "{:?}", applied_report.skipped);
            let mut merged = base.clone();
            merged.merge(&edited);
            prop_assert_eq!(&applied.objects, &merged.objects);

            // An unspellable segment is lifted once, where it was met, never again at an
            // ancestor that still holds it.
            let nameless: Vec<_> = report
                .lifted
                .iter()
                .filter_map(|lift| match lift {
                    ltk_meta::Lift::Nameless { object_hash, at, cause } => {
                        Some((*object_hash, at, cause.segment))
                    }
                    _ => None,
                })
                .collect();
            for (object, at, segment) in &nameless {
                prop_assert!(*segment < at.len());
                for (other_object, other, other_segment) in &nameless {
                    let is_ancestor = other.len() < at.len()
                        && other.segments() == &at.segments()[..other.len()];
                    prop_assert!(
                        !(object == other_object && is_ancestor && segment == other_segment),
                        "{} lifted again at {}", at, other
                    );
                }
            }
        }
    }

    /// A record is written only where the base holds something else, or nothing.
    #[test]
    fn no_record_writes_what_the_base_already_holds(
        base in common::bin(),
        edited in common::bin(),
    ) {
        let (patch, _) = base.diff(&edited, &common::names());
        for record in &patch.patches {
            if let Ok(existing) = base.resolve(record.object_hash, &record.path) {
                prop_assert_ne!(existing, &record.value, "{}", record.path);
            }
        }
    }

    /// With every name known, only a map insert or a changed shape lifts, and a lift-free diff
    /// takes no object the base already holds.
    #[test]
    fn with_every_name_known_only_inserts_and_shapes_lift(
        base in common::bin(),
        edited in common::bin(),
    ) {
        let (_, report) = base.diff(&edited, &common::names());
        for lift in &report.lifted {
            prop_assert!(
                matches!(lift, ltk_meta::Lift::MapInsert { .. } | ltk_meta::Lift::Mismatch { .. }),
                "{}", lift
            );
        }
        if report.lifted.is_empty() {
            for object in &report.objects {
                prop_assert!(!base.objects.contains_key(object));
            }
        }
    }
}

fn float_keyed(entries: &[(f32, i32)]) -> PropertyValueEnum {
    values::Map::new(
        ltk_meta::property::Kind::F32,
        ltk_meta::property::Kind::I32,
        entries
            .iter()
            .map(|(key, value)| (values::F32::new(*key).into(), int(*value)))
            .collect(),
    )
    .unwrap()
    .into()
}

#[test]
fn a_repeated_key_in_the_edit_diffs_as_the_merge_applies_it() {
    let base = one(vec![
        ("Lookup", lookup(&[(1, 1)])),
        ("Grown", lookup(&[(1, 1)])),
    ]);
    let edited = one(vec![
        ("Lookup", lookup(&[(1, 2), (1, 1)])),
        ("Grown", lookup(&[(2, 2), (2, 3)])),
    ]);

    let (patch, report) = base.diff(&edited, &names(&["Lookup", "Grown"]));

    let mut merged = base.clone();
    let merge_report = merged.merge(&edited);
    let inserted: usize = report
        .lifted
        .iter()
        .map(|lift| match lift {
            ltk_meta::Lift::MapInsert { keys, .. } => *keys,
            _ => 0,
        })
        .sum();
    assert_eq!(inserted, merge_report.keys_inserted);
    assert_applies_as_merge(&base, &edited, patch);
}

#[test]
fn a_float_key_the_resolver_cannot_tell_apart_lifts_its_map() {
    // `{-0}` resolves to the first key equal to it under `==`, which is `0.0`.
    let base = one(vec![("Floats", float_keyed(&[(0.0, 1), (-0.0, 2)]))]);
    let edited = one(vec![("Floats", float_keyed(&[(-0.0, 20)]))]);

    let (patch, report) = base.diff(&edited, &names(&["Floats"]));

    assert_eq!(
        patch.patches,
        [record("Floats", float_keyed(&[(0.0, 1), (-0.0, 20)]))]
    );
    assert!(matches!(
        report.lifted[..],
        [ltk_meta::Lift::Nameless { .. }]
    ));
    assert_applies_as_merge(&base, &edited, patch);
}

#[test]
fn a_nan_leaf_is_recorded_and_a_change_of_sign_is_not() {
    let base = one(vec![
        ("Nan", values::F32::new(f32::NAN).into()),
        ("Zero", values::F32::new(0.0).into()),
    ]);
    let edited = one(vec![
        ("Nan", values::F32::new(f32::NAN).into()),
        ("Zero", values::F32::new(-0.0).into()),
    ]);

    let (patch, _) = base.diff(&edited, &names(&["Nan", "Zero"]));

    assert_eq!(patch.patches.len(), 1);
    assert_eq!(patch.patches[0].path.as_str(), "Nan");
}

#[test]
fn siblings_under_one_unnamed_field_each_lift_at_their_own_position() {
    let base = one(vec![(
        "Secret",
        node(INNER, vec![("A", int(1)), ("B", int(1))]),
    )]);
    let edited = one(vec![(
        "Secret",
        node(INNER, vec![("A", int(2)), ("B", int(2))]),
    )]);

    let (patch, report) = base.diff(&edited, &names(&["A", "B"]));

    assert_eq!(report.objects, [BinHash(OBJECT)]);
    let lifted: Vec<_> = report
        .lifted
        .iter()
        .map(|lift| match lift {
            ltk_meta::Lift::Nameless { at, cause, .. } => (at.clone(), cause.segment),
            other => panic!("{other}"),
        })
        .collect();
    assert_eq!(
        lifted,
        [(at(&["Secret", "A"]), 0), (at(&["Secret", "B"]), 0)]
    );
    assert_applies_as_merge(&base, &edited, patch);
}
