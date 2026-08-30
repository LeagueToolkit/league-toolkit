//! Tests for walking a [`PropertyPath`] and applying `PTCH` records.

use std::io::Cursor;

use indexmap::IndexMap;
use insta::assert_ron_snapshot;
use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{
    path::{PatchError, PropertyPath, ResolveErrorKind, ValueShape},
    property::{values, Kind, NoMeta},
    Bin, BinObject, BinOverride, PropertyValueEnum,
};

/// A flipped minimap patch and the bin it patches, both from `UI.wad.client` of client
/// 16.16.804.9184.
const UIFLIPPED: &[u8] = include_bytes!("bins/lolminimap_uiflipped.ptch.bin");
const UIBASE: &[u8] = include_bytes!("bins/lolminimap_uibase.bin");

/// The object every test in the first half walks into.
const OBJECT: u32 = 0x0000_0001;

fn path(text: &str) -> PropertyPath {
    PropertyPath::new(text).unwrap()
}

fn hash(name: &str) -> BinHash {
    BinHash::hash_str(name)
}

/// Takes the value by value so `M` is pinned to [`NoMeta`] at the call site.
fn shape(value: PropertyValueEnum) -> ValueShape {
    ValueShape::of(&value)
}

fn properties(
    entries: impl IntoIterator<Item = (&'static str, PropertyValueEnum)>,
) -> IndexMap<BinHash, PropertyValueEnum> {
    entries
        .into_iter()
        .map(|(name, value)| (hash(name), value))
        .collect()
}

fn pointer(
    class: u32,
    entries: impl IntoIterator<Item = (&'static str, PropertyValueEnum)>,
) -> values::Struct {
    values::Struct {
        class_hash: class.into(),
        properties: properties(entries),
        meta: NoMeta,
    }
}

/// A tree with one value of every shape section 8.2 of the design has a rule for.
fn tree() -> Bin {
    let mut object = BinObject::<NoMeta>::new(OBJECT, 0x1000);
    object.properties = properties([
        ("Enabled", values::Bool::new(true).into()),
        (
            "Position",
            pointer(
                0x2000,
                [(
                    "UIRect",
                    values::Embedded(pointer(
                        0x3000,
                        [("Size", values::Vector2::default().into())],
                    ))
                    .into(),
                )],
            )
            .into(),
        ),
        // A pointer with a class hash of 0 is the format's null.
        ("Absent", pointer(0, []).into()),
        (
            "Elements",
            values::Container::from(vec![
                values::I32::new(10),
                values::I32::new(20),
                values::I32::new(30),
            ])
            .into(),
        ),
        (
            "Unordered",
            values::UnorderedContainer(values::Container::from(vec![values::String::from("a")]))
                .into(),
        ),
        (
            "Nested",
            values::Container::from(vec![pointer(
                0x4000,
                [("Deep", values::I32::new(7).into())],
            )])
            .into(),
        ),
        (
            "Maybe",
            values::Optional::from(values::F32::new(1.5)).into(),
        ),
        (
            "Nothing",
            values::Optional::empty(Kind::F32).unwrap().into(),
        ),
        (
            "Lookup",
            values::Map::new(
                Kind::Hash,
                Kind::String,
                vec![(
                    values::Hash::new(hash("weapon")).into(),
                    values::String::from("sword").into(),
                )],
            )
            .unwrap()
            .into(),
        ),
        (
            "Numbers",
            values::Map::new(
                Kind::U32,
                Kind::I32,
                vec![(values::U32::new(5).into(), values::I32::new(50).into())],
            )
            .unwrap()
            .into(),
        ),
    ]);

    Bin::new([object], std::iter::empty::<&str>())
}

#[track_caller]
fn resolves(text: &str, expected: PropertyValueEnum) {
    let bin = tree();
    let found = bin
        .resolve(OBJECT, &path(text))
        .unwrap_or_else(|e| panic!("{text} did not resolve: {e}"));
    assert_eq!(*found, expected, "{text}");
}

#[track_caller]
fn fails(text: &str, segment: usize, kind: ResolveErrorKind) {
    let bin = tree();
    let error = bin
        .resolve(OBJECT, &path(text))
        .expect_err(&format!("{text} resolved but should not have"));
    assert_eq!((error.segment(), error.kind()), (segment, kind), "{text}");
}

#[test]
fn descends_pointers_and_embeds() {
    resolves("Enabled", values::Bool::new(true).into());
    resolves("Position.UIRect.Size", values::Vector2::default().into());
}

#[test]
fn subscripts_lists_options_and_maps() {
    resolves("Elements[1]", values::I32::new(20).into());
    resolves("Unordered[0]", values::String::from("a").into());
    resolves("Maybe[0]", values::F32::new(1.5).into());
    resolves(r#"Lookup{"weapon"}"#, values::String::from("sword").into());
    resolves("Numbers{5}", values::I32::new(50).into());

    // The index is a `strtol` token, so hex and octal reach the same element.
    resolves("Elements[0x2]", values::I32::new(30).into());
    resolves("Elements[02]", values::I32::new(30).into());
}

/// A subscript leaves the cursor on the element, which the next segment descends into.
#[test]
fn continues_past_a_subscript() {
    resolves("Nested[0].Deep", values::I32::new(7).into());
}

#[test]
fn reports_a_missing_object() {
    let bin = tree();
    let error = bin.resolve(0xdead_beef_u32, &path("Enabled")).unwrap_err();
    assert_eq!(error.segment(), 0);
    assert_eq!(
        error.kind(),
        ResolveErrorKind::MissingObject(0xdead_beef_u32.into())
    );
}

#[test]
fn reports_a_missing_property() {
    fails("Nope", 0, ResolveErrorKind::MissingProperty(hash("Nope")));
    fails(
        "Position.Nope",
        1,
        ResolveErrorKind::MissingProperty(hash("Nope")),
    );
}

#[test]
fn reports_a_null_pointer() {
    fails("Absent.Anything", 1, ResolveErrorKind::NullPointer);
}

/// The segment that could not be applied is the one reported, not the leaf before it.
#[test]
fn reports_what_cannot_be_descended_into() {
    fails(
        "Enabled.Size",
        1,
        ResolveErrorKind::CannotDescend(Kind::Bool),
    );
    fails(
        "Elements.Size",
        1,
        ResolveErrorKind::CannotDescend(Kind::Container),
    );
    fails(
        "Maybe.Size",
        1,
        ResolveErrorKind::CannotDescend(Kind::Optional),
    );
    fails("Lookup.Size", 1, ResolveErrorKind::CannotDescend(Kind::Map));
}

#[test]
fn reports_a_subscript_the_value_does_not_take() {
    fails("Enabled[0]", 0, ResolveErrorKind::NotIndexable(Kind::Bool));
    fails(
        "Position[0]",
        0,
        ResolveErrorKind::NotIndexable(Kind::Struct),
    );
    // Right idea, wrong flavour: a list is indexed, a map is keyed.
    fails(
        r#"Elements{"a"}"#,
        0,
        ResolveErrorKind::NotIndexable(Kind::Container),
    );
    fails("Lookup[0]", 0, ResolveErrorKind::NotIndexable(Kind::Map));
}

#[test]
fn reports_an_index_out_of_range() {
    fails(
        "Elements[9]",
        0,
        ResolveErrorKind::IndexOutOfRange { index: 9, len: 3 },
    );
    // An option holds at most one value, so only `[0]` can ever land.
    fails(
        "Maybe[1]",
        0,
        ResolveErrorKind::IndexOutOfRange { index: 1, len: 1 },
    );
    fails(
        "Nothing[0]",
        0,
        ResolveErrorKind::IndexOutOfRange { index: 0, len: 0 },
    );
}

#[test]
fn reports_a_key_that_does_not_fit_or_does_not_exist() {
    // A bool literal has no conversion to a hash key.
    fails("Lookup{true}", 0, ResolveErrorKind::InvalidKey(Kind::Hash));
    fails(
        r#"Numbers{"five"}"#,
        0,
        ResolveErrorKind::InvalidKey(Kind::U32),
    );

    fails(r#"Lookup{"shield"}"#, 0, ResolveErrorKind::KeyNotFound);
    fails("Numbers{6}", 0, ResolveErrorKind::KeyNotFound);
}

/// A slot knows whether the thing holding it declared a kind.
#[test]
fn pins_a_slot_only_where_a_container_holds_it() {
    let mut bin = tree();

    let free = bin.resolve_mut(OBJECT, &path("Enabled")).unwrap();
    assert_eq!(free.pinned_kind(), None);

    let item = bin.resolve_mut(OBJECT, &path("Elements[1]")).unwrap();
    assert_eq!(item.pinned_kind(), Some(Kind::I32));

    let entry = bin
        .resolve_mut(OBJECT, &path(r#"Lookup{"weapon"}"#))
        .unwrap();
    assert_eq!(entry.pinned_kind(), Some(Kind::String));

    let mut inside = bin.resolve_mut(OBJECT, &path("Maybe[0]")).unwrap();
    assert_eq!(inside.pinned_kind(), Some(Kind::F32));
    assert!(inside.set(values::I32::new(1).into()).is_err());
    assert_eq!(
        inside.set(values::F32::new(2.5).into()).unwrap(),
        values::F32::new(1.5).into()
    );
    resolves_in(&bin, "Maybe[0]", values::F32::new(2.5).into());
}

#[track_caller]
fn resolves_in(bin: &Bin, text: &str, expected: PropertyValueEnum) {
    assert_eq!(
        *bin.resolve(OBJECT, &path(text)).unwrap(),
        expected,
        "{text}"
    );
}

#[test]
fn patch_replaces_a_leaf_of_the_same_shape() {
    let mut bin = tree();

    let replaced = bin
        .patch(OBJECT, &path("Enabled"), values::Bool::new(false).into())
        .unwrap();
    assert_eq!(replaced, Some(values::Bool::new(true).into()));
    resolves_in(&bin, "Enabled", values::Bool::new(false).into());

    let replaced = bin
        .patch(OBJECT, &path("Elements[1]"), values::I32::new(99).into())
        .unwrap();
    assert_eq!(replaced, Some(values::I32::new(20).into()));
    resolves_in(&bin, "Elements[1]", values::I32::new(99).into());
}

/// The insert case, which 2,455 of Riot's shipped records need.
#[test]
fn patch_creates_a_leaf_the_base_does_not_serialize() {
    let mut bin = tree();

    assert_eq!(
        bin.patch(OBJECT, &path("FlipX"), values::Bool::new(true).into())
            .unwrap(),
        None
    );
    resolves_in(&bin, "FlipX", values::Bool::new(true).into());

    // Also inside a pointer that is already there.
    assert_eq!(
        bin.patch(
            OBJECT,
            &path("Position.UIRect.Offset"),
            values::Vector2::default().into()
        )
        .unwrap(),
        None
    );

    // But never through a subscript, which needs something to subscript.
    let error = bin
        .patch(OBJECT, &path("Missing[0]"), values::I32::new(1).into())
        .unwrap_err();
    assert!(matches!(
        error,
        PatchError::Resolve(e) if e.kind() == ResolveErrorKind::MissingProperty(hash("Missing"))
    ));
}

#[test]
fn patch_rejects_a_different_shape_and_changes_nothing() {
    let mut bin = tree();

    let error = bin
        .patch(OBJECT, &path("Enabled"), values::I32::new(1).into())
        .unwrap_err();
    assert_eq!(
        error,
        PatchError::TypeMismatch {
            expected: shape(values::Bool::new(true).into()),
            found: shape(values::I32::new(1).into()),
        }
    );
    assert_eq!(
        error.to_string(),
        "type mismatch: the property is Bool, the patch carries I32"
    );
    resolves_in(&bin, "Enabled", values::Bool::new(true).into());

    // A container compares by what it holds, not just by being a container.
    let error = bin
        .patch(
            OBJECT,
            &path("Elements"),
            values::Container::from(vec![values::F32::new(1.0)]).into(),
        )
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "type mismatch: the property is Container[I32], the patch carries Container[F32]"
    );
    resolves_in(&bin, "Elements[0]", values::I32::new(10).into());
}

/// An embed's class is compared exactly, the way the client compares `MetaClass` pointers.
#[test]
fn patch_compares_an_embed_class_but_not_a_pointer_class() {
    let mut bin = tree();

    let wrong_class = values::Embedded(pointer(0xbeef, [])).into();
    assert!(bin
        .patch(OBJECT, &path("Position.UIRect"), wrong_class)
        .is_err());

    let right_class = values::Embedded(pointer(0x3000, [])).into();
    assert!(bin
        .patch(OBJECT, &path("Position.UIRect"), right_class)
        .is_ok());

    // A pointer's class is not compared at all: without the class hierarchy there is no way to
    // tell a descendant, which the client accepts, from an unrelated class, which it rejects.
    let other_class = pointer(0x9999, []).into();
    assert!(bin.patch(OBJECT, &path("Position"), other_class).is_ok());
}

fn uiflipped() -> BinOverride {
    BinOverride::from_reader(&mut Cursor::new(UIFLIPPED)).expect("the patch fixture reads")
}

fn uibase() -> Bin {
    Bin::from_reader(&mut Cursor::new(UIBASE)).expect("the base fixture reads")
}

/// Every record of a shipped patch, checked against the bin it ships next to.
#[test]
fn checks_a_shipped_patch_against_its_base() {
    let report = uiflipped().check(&uibase());

    assert_ron_snapshot!(format!("{report}"));
    assert!(report.is_clean(), "{:#?}", report.skipped);
    assert_eq!(report.applied, uiflipped().patches.len());
}

/// `check` predicts `apply`.
#[test]
fn applies_a_shipped_patch_to_its_base() {
    let mut base = uibase();
    let predicted = uiflipped().check(&base);
    let report = uiflipped().apply(&mut base);

    assert_eq!(report, predicted);
    assert!(report.is_clean());
}

/// The patch is the flipped minimap, so the values it moves are the ones that flip.
#[test]
fn applying_the_flipped_minimap_moves_what_it_says_it_moves() {
    let mut base = uibase();

    // `VoiceChatPanel_ButtonClicked` is anchored to the right edge before the patch.
    let anchor = path("Position.Anchors.Anchor");
    assert_eq!(
        *base.resolve(0x4a47_c414_u32, &anchor).unwrap(),
        values::Vector2::new(glam::Vec2::new(1.0, 1.0)).into()
    );

    // `MinimapFrame` does not serialize `FlipX` at all, so the record has to create it.
    let flip = path("FlipX");
    assert!(base.resolve(0xa4ed_cb0d_u32, &flip).is_err());

    uiflipped().apply(&mut base);

    assert_eq!(
        *base.resolve(0x4a47_c414_u32, &anchor).unwrap(),
        values::Vector2::new(glam::Vec2::new(0.0, 1.0)).into()
    );
    assert_eq!(
        *base.resolve(0xa4ed_cb0d_u32, &flip).unwrap(),
        values::Bool::new(true).into()
    );
}

/// A patch cannot reach an object that is not in the bin it is laid over.
#[test]
fn skips_a_record_whose_object_is_not_there() {
    let patch_bin = BinOverride::builder()
        .set(0xdead_beef_u32, path("Enabled"), values::Bool::new(true))
        .build();

    let mut base = tree();
    let report = patch_bin.clone().check(&base);
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped.len(), 1);
    assert!(matches!(
        report.skipped[0].error,
        PatchError::Resolve(e)
            if e.segment() == 0
            && e.kind() == ResolveErrorKind::MissingObject(0xdead_beef_u32.into())
    ));

    assert_eq!(patch_bin.apply(&mut base), report);
}

/// The delete list runs first, then the patch's own objects, then the records - so a record can
/// address an object the same patch just added.
#[test]
fn applies_deletions_objects_and_records_in_order() {
    let mut base = tree();
    base.objects
        .insert(0x0002_u32.into(), BinObject::new(0x0002_u32, 0x1000));

    let patch_bin = BinOverride::builder()
        .delete(0x0002_u32)
        .object(
            BinObject::<NoMeta>::builder(0x0003_u32, 0x1000)
                .property(hash("Enabled"), values::Bool::new(false))
                .build(),
        )
        .set(0x0003_u32, path("Enabled"), values::Bool::new(true))
        .build();

    let report = patch_bin.apply(&mut base);

    assert_eq!(report.deleted, [BinHash::from(0x0002_u32)]);
    assert_eq!(report.added, [BinHash::from(0x0003_u32)]);
    assert!(report.replaced.is_empty());
    assert_eq!((report.applied, report.inserted), (1, 0));
    assert!(report.is_clean());

    assert!(!base.objects.contains_key(&BinHash::from(0x0002_u32)));
    assert_eq!(
        *base.resolve(0x0003_u32, &path("Enabled")).unwrap(),
        values::Bool::new(true).into()
    );
}
