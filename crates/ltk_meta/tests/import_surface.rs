//! The streaming surface, imported the way a consumer is meant to import it.
//!
//! Two paths appear in this file and no more: `ltk_meta::concrete` for the constructors Rust
//! cannot infer - `mount` and `LruObjectCache::new` are expression position, where the
//! `M = NoMeta` default never applies - and the crate root for everything else. The path
//! `ltk_meta::stream` must not appear here at all: needing it is the two-import-site spread the
//! root re-exports removed, and this file is what catches it coming back.
//!
//! The handle is mounted with no turbofish and no annotation, which is the other half of the
//! assertion: `R` and `M` are inferred once and carry through every cursor and view it hands out.

use std::{io::Cursor, num::NonZeroUsize, sync::Arc};

use ltk_meta::{
    concrete::{BinStream, LruObjectCache, NoCache},
    BatchObjects, BinToc, ContainerItems, ContainerView, Entries, Error, MapEntries, MapView,
    Numbering, ObjectCache, ObjectEntry, ObjectStream, ObjectView, Objects, OptionalView,
    Properties, PropertyView, StructView, ValueView,
};

/// Path hash of the fixture's only object.
const PATH_HASH: u32 = 0x8066_f665;
/// Its class, `VfxSystemDefinitionData`.
const CLASS_HASH: u32 = 0x45cd_899f;
/// Its first property: a container holding one emitter definition.
const EMITTERS: u32 = 0x868e_b76a;
/// The class of that emitter.
const EMITTER_CLASS: u32 = 0x09cd_e442;

/// A consumer that keeps its cache policy rather than handing it straight over.
///
/// The field is the strict case: `set_cache` lets an argument infer `M`, but a field has no
/// inference to lean on, so `ObjectCache`'s own `M = NoMeta` default is what keeps this to one
/// import instead of three.
struct CacheHolder {
    policy: Box<dyn ObjectCache + Send>,
}

/// What a full descent met, so the walk asserts something beyond having compiled.
#[derive(Debug, Default, PartialEq, Eq)]
struct Tally {
    structs: usize,
    containers: usize,
    maps: usize,
    optionals: usize,
    strings: usize,
}

/// Descends one value to the bottom, counting what it meets.
///
/// The four sub-views a `ValueView` opens into are named in the helpers' parameter lists rather
/// than behind a turbofish, which is what proves the root re-exports carry `M = NoMeta` with them.
fn walk_value(value: ValueView<'_>, tally: &mut Tally) -> Result<(), Error> {
    match value {
        ValueView::String(_) => tally.strings += 1,
        ValueView::Container(list) | ValueView::UnorderedContainer(list) => {
            walk_container(list, tally)?;
        }
        ValueView::Map(map) => walk_map(map, tally)?,
        ValueView::Optional(option) => walk_optional(option, tally)?,
        ValueView::Struct(nested) | ValueView::Embedded(nested) => walk_struct(nested, tally)?,
        // Listed rather than caught by `_`: a new composite view must fail here until it is
        // named at the root too.
        ValueView::None
        | ValueView::Bool(_)
        | ValueView::I8(_)
        | ValueView::U8(_)
        | ValueView::I16(_)
        | ValueView::U16(_)
        | ValueView::I32(_)
        | ValueView::U32(_)
        | ValueView::I64(_)
        | ValueView::U64(_)
        | ValueView::F32(_)
        | ValueView::Vector2(_)
        | ValueView::Vector3(_)
        | ValueView::Vector4(_)
        | ValueView::Matrix44(_)
        | ValueView::Color(_)
        | ValueView::Hash(_)
        | ValueView::WadChunkLink(_)
        | ValueView::ObjectLink(_)
        | ValueView::BitBool(_) => {}
    }
    Ok(())
}

fn walk_struct(view: StructView<'_>, tally: &mut Tally) -> Result<(), Error> {
    tally.structs += 1;
    for property in view.properties() {
        walk_value(property?.value_view()?, tally)?;
    }
    Ok(())
}

fn walk_container(view: ContainerView<'_>, tally: &mut Tally) -> Result<(), Error> {
    tally.containers += 1;
    let items: ContainerItems<'_> = view.iter();
    for item in items {
        walk_value(item?, tally)?;
    }
    Ok(())
}

fn walk_map(view: MapView<'_>, tally: &mut Tally) -> Result<(), Error> {
    tally.maps += 1;
    let entries: MapEntries<'_> = view.iter();
    for entry in entries {
        let (key, value) = entry?;
        walk_value(key, tally)?;
        walk_value(value, tally)?;
    }
    Ok(())
}

fn walk_optional(view: OptionalView<'_>, tally: &mut Tally) -> Result<(), Error> {
    tally.optionals += 1;
    if let Some(value) = view.get()? {
        walk_value(value, tally)?;
    }
    Ok(())
}

#[test]
fn a_streaming_walk_needs_only_the_root_and_concrete() -> Result<(), Error> {
    let mut stream = BinStream::mount(Cursor::new(include_bytes!("bins/leona_small.bin")))?;

    assert_eq!(stream.version(), 3);
    assert_eq!(stream.class_hashes().len(), 1);

    let mut tally = Tally::default();
    let mut swept = 0;
    let mut objects = stream.objects();

    while let Some(mut object) = objects.next()? {
        swept += 1;
        assert_eq!(object.path_hash(), PATH_HASH.into());
        assert_eq!(object.class_hash(), CLASS_HASH.into());

        let view = object.view()?;
        assert_eq!(view.property_count(), 5);

        for property in view.properties() {
            walk_value(property?.value_view()?, &mut tally)?;
        }
    }

    assert_eq!(swept, 1, "the fixture holds one object");
    assert_eq!(
        tally,
        Tally {
            // The container's one emitter, plus the struct embedded in it.
            structs: 2,
            containers: 1,
            maps: 0,
            optionals: 2,
            // Three on the object, three inside the emitter.
            strings: 6,
        }
    );

    Ok(())
}

#[test]
fn the_cache_policies_are_named_from_concrete() -> Result<(), Error> {
    let mut stream = BinStream::mount(Cursor::new(include_bytes!("bins/leona_small.bin")))?;

    // `new` is expression position, so the capacity comes through the `concrete` alias - and the
    // annotation says the alias and the root re-export are one type.
    let lru: ltk_meta::LruObjectCache =
        LruObjectCache::new(NonZeroUsize::new(4).expect("4 is not zero"));
    stream.set_cache(Box::new(lru));

    let parsed = stream
        .cached_object(PATH_HASH)?
        .expect("the fixture's only object");
    assert_eq!(parsed.properties.len(), 5);

    let hit = stream
        .cached_object(PATH_HASH)?
        .expect("the same object, from the cache");
    assert!(Arc::ptr_eq(&parsed, &hit), "the LRU served the second call");

    // `NoCache` keeps nothing, so the next call parses a fresh object rather than hitting.
    // Named in expression position straight from `concrete`, which is why it is re-exported
    // there rather than aliased.
    let holder = CacheHolder {
        policy: Box::new(NoCache),
    };
    stream.set_cache(holder.policy);

    let again = stream
        .cached_object(PATH_HASH)?
        .expect("the object, parsed again");
    assert!(!Arc::ptr_eq(&parsed, &again), "NoCache has nothing to hit");

    Ok(())
}

/// Every remaining streaming name, spelled at the crate root in type position.
///
/// The bindings are annotated on purpose here: a name the root stops exporting, or one whose
/// metadata default stops applying through the re-export, fails to compile.
#[test]
fn every_streaming_name_is_spelled_at_the_crate_root() -> Result<(), Error> {
    let mut stream = BinStream::mount(Cursor::new(include_bytes!("bins/leona_small.bin")))?;

    let numbering: Numbering = stream.numbering();
    assert_eq!(numbering, Numbering::Current);

    let entries: Entries<'_, _> = stream.entries();
    let descriptors = entries.collect::<Result<Vec<ObjectEntry>, Error>>()?;
    assert_eq!(descriptors.len(), 1);
    assert_eq!(descriptors[0].path_hash, PATH_HASH.into());

    let toc: &BinToc = stream.toc()?;
    assert_eq!(toc.entries().len(), 1);

    let mut objects: Objects<'_, _> = stream.objects();
    let mut object: ObjectStream<'_, _> = objects.next()?.expect("the fixture's only object");

    let view: ObjectView<'_> = object.view()?;
    let mut properties: Properties<'_> = view.properties();
    let emitters: PropertyView<'_> = properties.next().expect("a first property")?;
    assert_eq!(emitters.name_hash(), EMITTERS.into());

    let ValueView::Container(list) = emitters.value_view()? else {
        panic!("{EMITTERS:#010x} is the container of emitter definitions");
    };
    assert_eq!(list.len(), 1);

    let Some(ValueView::Struct(emitter)) = list.get(0)? else {
        panic!("the container holds one emitter struct");
    };
    assert_eq!(emitter.class_hash(), EMITTER_CLASS.into());

    let mut batch: BatchObjects<'_, _> = stream.objects_batch([PATH_HASH, 0xdead_beef_u32]);
    let mut found = 0;
    while let Some(object) = batch.next()? {
        found += 1;
        assert_eq!(object.path_hash(), PATH_HASH.into());
    }
    assert_eq!(found, 1);
    assert_eq!(batch.missing().len(), 1, "0xdeadbeef is not in this bin");

    Ok(())
}
