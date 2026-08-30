use std::{
    cell::Cell,
    io::{self, Read, Seek, SeekFrom},
    num::NonZeroUsize,
    rc::Rc,
    sync::Arc,
};

use glam::{Mat4, Vec2, Vec3, Vec4};
use ltk_hash::BinHash;
use ltk_primitives::Color;

use crate::{
    concrete,
    concrete::values,
    path::ValueShape,
    property::values::{Embedded, UnorderedContainer},
    stream::{layout::Numbering, LruObjectCache, ValueView},
    traits::PropertyExt as _,
    Bin, BinKind, BinObject, Error, PropertyKind, PropertyValueEnum,
};

type BinStream<R> = concrete::BinStream<R>;

/// A three-object bin with a dependency and a spread of property kinds, as bytes.
fn sample_bin() -> (Bin, Vec<u8>) {
    let bin = Bin::builder()
        .dependency("common.bin")
        .object(
            concrete::BinObject::builder(0x1111_0001u32, 0xAAAA_0001u32)
                .property(0x0001u32, values::I32::new(42))
                .property(0x0002u32, values::String::from("hello"))
                .build(),
        )
        .object(
            concrete::BinObject::builder(0x1111_0002u32, 0xAAAA_0002u32)
                .property(
                    0x0003u32,
                    values::Container::from(vec![values::F32::new(1.5), values::F32::new(2.5)]),
                )
                .build(),
        )
        .object(concrete::BinObject::builder(0x1111_0003u32, 0xAAAA_0003u32).build())
        .build();

    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    (bin, cursor.into_inner())
}

/// One property of every kind, complex kinds included, with distinct name hashes.
///
/// `Elements` (`0x0016`) is a container of embeds, each with a `Position`, which is what the
/// nested-descent tests walk.
fn every_kind() -> Vec<(BinHash, PropertyValueEnum)> {
    let embed = |x: f32| {
        Embedded(values::Struct {
            class_hash: 0x0BED_0000u32.into(),
            properties: [(POSITION, values::Vector2::new(Vec2::new(x, -x)).into())]
                .into_iter()
                .collect(),
            meta: Default::default(),
        })
    };

    let values: Vec<PropertyValueEnum> = vec![
        values::None::default().into(),
        values::Bool::new(true).into(),
        values::BitBool::new(true).into(),
        values::I8::new(-1).into(),
        values::U8::new(1).into(),
        values::I16::new(-2).into(),
        values::U16::new(2).into(),
        values::I32::new(-3).into(),
        values::U32::new(3).into(),
        values::I64::new(-4).into(),
        values::U64::new(4).into(),
        values::F32::new(1.5).into(),
        values::Vector2::new(Vec2::ONE).into(),
        values::Vector3::new(Vec3::ONE).into(),
        values::Vector4::new(Vec4::ONE).into(),
        values::Matrix44::new(Mat4::IDENTITY).into(),
        values::Color::new(Color::new(1, 2, 3, 4)).into(),
        values::String::from("a string").into(),
        values::Hash::new(0xABCDu32).into(),
        values::WadChunkLink::new(0xDEAD_BEEFu64).into(),
        values::ObjectLink::new(0x4444u32).into(),
        embed(1.0).0.into(),
        // A null pointer: class hash 0, no size field, no body.
        values::Struct::default().into(),
        embed(5.0).into(),
        values::Container::from(vec![embed(0.0), embed(1.0), embed(2.0), embed(3.0)]).into(),
        UnorderedContainer(values::Container::from(vec![values::U8::new(9)])).into(),
        values::Container::from(vec![
            values::String::from("one"),
            values::String::from("two"),
            values::String::from("three"),
        ])
        .into(),
        values::Optional::from(values::F32::new(2.5)).into(),
        values::Optional::empty(PropertyKind::F32)
            .expect("F32 nests")
            .into(),
        values::Map::new(
            PropertyKind::U32,
            PropertyKind::String,
            vec![
                (
                    values::U32::new(7).into(),
                    values::String::from("seven").into(),
                ),
                (
                    values::U32::new(8).into(),
                    values::String::from("eight").into(),
                ),
            ],
        )
        .expect("a valid map")
        .into(),
    ];

    values
        .into_iter()
        .enumerate()
        .map(|(index, value)| (BinHash(index as u32), value))
        .collect()
}

/// The name hash of the `Position` inside each embed of `Elements`.
const POSITION: BinHash = BinHash(0xF005);
/// The name hash [`every_kind`] gives the container of embeds.
const ELEMENTS: BinHash = BinHash(24);
/// The name hash [`every_kind`] gives the container of strings.
const STRINGS: BinHash = BinHash(26);

/// A one-object bin holding [`every_kind`], and its bytes.
fn every_kind_bin() -> (Bin, Vec<u8>) {
    let mut object = BinObject::new(0x2222_0001u32, 0xBBBB_0001u32);
    for (name_hash, value) in every_kind() {
        object.insert(name_hash, value);
    }

    let bin = Bin::new([object], std::iter::empty::<&str>());
    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    (bin, cursor.into_inner())
}

/// The one object of an [`every_kind_bin`], by hash.
fn only_object(bin: &Bin) -> BinHash {
    *bin.objects.keys().next().expect("one object")
}

/// Counts the reads and position-changing seeks that reach the wrapped source.
struct Counting<R> {
    inner: R,
    reads: Rc<Cell<usize>>,
    moving_seeks: Rc<Cell<usize>>,
}

impl<R: Seek> Counting<R> {
    fn new(inner: R) -> (Self, Rc<Cell<usize>>, Rc<Cell<usize>>) {
        let reads = Rc::new(Cell::new(0));
        let moving_seeks = Rc::new(Cell::new(0));
        (
            Self {
                inner,
                reads: Rc::clone(&reads),
                moving_seeks: Rc::clone(&moving_seeks),
            },
            reads,
            moving_seeks,
        )
    }
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads.set(self.reads.get() + 1);
        self.inner.read(buf)
    }
}

impl<R: Seek> Seek for Counting<R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let before = self.inner.stream_position()?;
        let after = self.inner.seek(pos)?;
        if after != before {
            self.moving_seeks.set(self.moving_seeks.get() + 1);
        }
        Ok(after)
    }
}

#[test]
fn mount_exposes_the_header_without_seeking_past_it() {
    let (bin, bytes) = sample_bin();

    let (source, _reads, moving_seeks) = Counting::new(io::Cursor::new(&bytes));
    let mut stream = BinStream::mount(source).expect("the stream mounts");

    assert_eq!(stream.version(), 3);
    assert_eq!(stream.dependencies(), ["common.bin"]);
    assert_eq!(
        stream.class_hashes(),
        bin.objects
            .values()
            .map(|o| o.class_hash)
            .collect::<Vec<_>>()
    );
    assert_eq!(moving_seeks.get(), 0, "mount moved the reader");

    // The header really is all mount needs: a file cut off at the first object's size field
    // still mounts and answers the same.
    let first_object = stream.entries().next().expect("an entry").expect("reads");
    let mut truncated = BinStream::mount(io::Cursor::new(&bytes[..first_object.offset as usize]))
        .expect("the truncated header mounts");
    assert_eq!(truncated.version(), stream.version());
    assert_eq!(truncated.dependencies(), stream.dependencies());
    assert_eq!(truncated.class_hashes(), stream.class_hashes());
    assert!(truncated.entries().next().expect("an entry").is_err());
}

#[test]
fn refuses_the_wrong_magic() {
    assert!(matches!(
        BinStream::mount(io::Cursor::new(&b"PTCH\x01\0\0\0"[..])),
        Err(Error::UnexpectedBinKind {
            expected: BinKind::Prop,
            found: BinKind::Override,
        })
    ));
    assert!(matches!(
        BinStream::mount(io::Cursor::new(&b"OEGM\0\0\0\0"[..])),
        Err(Error::InvalidFileSignature)
    ));
}

#[test]
fn entries_harvest_what_the_eager_parse_holds() {
    let (bin, bytes) = sample_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let entries: Vec<_> = stream
        .entries()
        .collect::<Result<_, _>>()
        .expect("every entry reads");

    assert_eq!(
        entries
            .iter()
            .map(|e| (e.path_hash, e.class_hash))
            .collect::<Vec<_>>(),
        bin.objects
            .values()
            .map(|o| (o.path_hash, o.class_hash))
            .collect::<Vec<_>>()
    );

    // The declared ranges tile the object table exactly: each row starts where the previous
    // one ends, and the last ends at the end of the file.
    for pair in entries.windows(2) {
        assert_eq!(pair[0].byte_range().end, pair[1].offset);
    }
    assert_eq!(
        entries.last().expect("three entries").byte_range().end,
        bytes.len() as u64
    );
}

#[test]
fn a_repeated_sweep_reuses_the_toc() {
    let (_, bytes) = sample_bin();

    let (source, reads, moving_seeks) = Counting::new(io::Cursor::new(&bytes));
    let mut stream = BinStream::mount(source).expect("the stream mounts");

    let first: Vec<_> = stream
        .entries()
        .collect::<Result<_, _>>()
        .expect("the first sweep reads");
    let (reads_after_sweep, seeks_after_sweep) = (reads.get(), moving_seeks.get());

    // The sweep populated the TOC as a side effect; nothing after it touches the source.
    let toc = stream.toc().expect("the TOC is already built").clone();
    let second: Vec<_> = stream
        .entries()
        .collect::<Result<_, _>>()
        .expect("the second sweep reads");

    assert_eq!(toc.entries(), first);
    assert_eq!(second, first);
    assert_eq!(reads.get(), reads_after_sweep, "a second harvest pass ran");
    assert_eq!(moving_seeks.get(), seeks_after_sweep);
}

#[test]
fn objects_are_found_by_path_hash() {
    let (bin, bytes) = sample_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");

    let mut object = stream
        .object(0x1111_0002u32)
        .expect("the TOC builds")
        .expect("the object exists");
    assert_eq!(object.path_hash(), 0x1111_0002u32.into());
    assert_eq!(object.class_hash(), 0xAAAA_0002u32.into());
    assert_eq!(object.property_count().expect("the count reads"), 1);
    assert_eq!(object.entry().byte_range(), object.byte_range());

    assert!(stream
        .object(0xDEAD_BEEFu32)
        .expect("the TOC is cached")
        .is_none());

    // The cursor agrees with the eager parse, property counts included.
    let mut objects = stream.objects();
    let mut seen = 0;
    while let Some(mut object) = objects.next().expect("the cursor advances") {
        let eager = &bin.objects[&object.path_hash()];
        assert_eq!(object.class_hash(), eager.class_hash);
        assert_eq!(
            object.property_count().expect("the count reads") as usize,
            eager.properties.len()
        );
        seen += 1;
    }
    assert_eq!(seen, bin.objects.len());
}

// =============================================================================
// Views
// =============================================================================

#[test]
fn a_view_decodes_every_kind_the_way_the_eager_reader_does() {
    let (bin, bytes) = every_kind_bin();
    let eager = &bin.objects[&only_object(&bin)];

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(eager.path_hash)
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    assert_eq!(view.path_hash(), eager.path_hash);
    assert_eq!(view.class_hash(), eager.class_hash);
    assert_eq!(view.property_count() as usize, eager.properties.len());
    assert_eq!(view.numbering(), Numbering::Current);

    let mut seen = 0;
    for (property, (name_hash, expected)) in view.properties().zip(eager.properties.iter()) {
        let property = property.expect("the property reads");

        assert_eq!(property.name_hash(), *name_hash);
        assert_eq!(property.kind(), expected.kind());
        assert_eq!(
            property.raw().len(),
            expected.size_no_header(),
            "{:?}: the viewed bytes are not the value's serialized size",
            expected.kind()
        );
        assert_eq!(
            property.shape().expect("the shape reads"),
            ValueShape::of(expected),
            "{:?}: the wire shape and ValueShape::of disagree",
            expected.kind()
        );
        assert_eq!(
            &property.value().expect("the value decodes"),
            expected,
            "{:?}: the owned decode disagrees with the eager parse",
            expected.kind()
        );
        assert_eq!(
            property.value_view().expect("the value views").kind(),
            expected.kind()
        );
        seen += 1;
    }
    assert_eq!(seen, eager.properties.len());
}

#[test]
fn a_view_finds_a_property_by_name_hash() {
    let (bin, bytes) = every_kind_bin();
    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    let strings = view
        .property(STRINGS)
        .expect("the walk reaches it")
        .expect("the property exists");
    assert_eq!(strings.kind(), PropertyKind::Container);
    assert_eq!(strings.item_count().expect("the count reads"), Some(3));

    assert!(view
        .property(0xDEAD_BEEFu32)
        .expect("the walk runs")
        .is_none());

    // Only containers and maps are counted; an option reports its own presence instead.
    assert_eq!(
        view.property(11u32)
            .expect("the walk runs")
            .expect("property 11 is the f32")
            .item_count()
            .expect("the count reads"),
        None
    );
}

#[test]
fn leaves_carry_their_decoded_values() {
    let (bin, bytes) = every_kind_bin();
    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    let leaf = |name_hash: u32| {
        view.property(name_hash)
            .expect("the walk runs")
            .expect("the property exists")
            .value_view()
            .expect("the value views")
    };

    assert!(matches!(leaf(0), ValueView::None));
    assert!(matches!(leaf(1), ValueView::Bool(true)));
    assert!(matches!(leaf(2), ValueView::BitBool(true)));
    assert!(matches!(leaf(3), ValueView::I8(-1)));
    assert!(matches!(leaf(11), ValueView::F32(v) if v == 1.5));
    assert!(matches!(leaf(12), ValueView::Vector2(v) if v == Vec2::ONE));
    assert!(matches!(leaf(15), ValueView::Matrix44(v) if v == Mat4::IDENTITY));
    assert!(matches!(leaf(16), ValueView::Color(c) if c == Color::new(1, 2, 3, 4)));
    assert!(matches!(leaf(17), ValueView::String("a string")));
    assert!(matches!(leaf(18), ValueView::Hash(h) if h == 0xABCDu32.into()));
    assert!(matches!(leaf(19), ValueView::WadChunkLink(h) if h == 0xDEAD_BEEFu64.into()));
    assert!(matches!(leaf(20), ValueView::ObjectLink(h) if h == 0x4444u32.into()));

    // A null pointer has no body at all, so its view has no properties.
    let ValueView::Struct(null) = leaf(22) else {
        panic!("property 22 is a null pointer");
    };
    assert_eq!(*null.class_hash(), 0);
    assert_eq!(null.property_count(), 0);
    assert_eq!(null.properties().count(), 0);

    let ValueView::Optional(empty) = leaf(28) else {
        panic!("property 28 is an empty option");
    };
    assert!(empty.is_none());
    assert_eq!(empty.item_kind(), PropertyKind::F32);
    assert!(empty.get().expect("the option reads").is_none());

    let ValueView::Optional(full) = leaf(27) else {
        panic!("property 27 is a filled option");
    };
    assert!(full.is_some());
    assert!(matches!(full.get().expect("the option reads"), Some(ValueView::F32(v)) if v == 2.5));
}

#[test]
fn a_map_view_walks_its_entries() {
    let (bin, bytes) = every_kind_bin();
    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    let ValueView::Map(map) = view
        .property(29u32)
        .expect("the walk runs")
        .expect("property 29 is the map")
        .value_view()
        .expect("the value views")
    else {
        panic!("property 29 is a map");
    };

    assert_eq!(map.key_kind(), PropertyKind::U32);
    assert_eq!(map.value_kind(), PropertyKind::String);
    assert_eq!(map.len(), 2);
    assert!(!map.is_empty());

    let entries: Vec<_> = map
        .iter()
        .collect::<Result<_, _>>()
        .expect("every entry reads");
    assert!(matches!(
        entries[0],
        (ValueView::U32(7), ValueView::String("seven"))
    ));
    assert!(matches!(
        entries[1],
        (ValueView::U32(8), ValueView::String("eight"))
    ));
}

/// `Elements[3].Position` is one index and one lookup, and nothing beside them is decoded.
#[test]
fn descent_reaches_a_nested_leaf_without_touching_its_siblings() {
    let (bin, bytes) = every_kind_bin();
    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    let ValueView::Container(elements) = view
        .property(ELEMENTS)
        .expect("the walk runs")
        .expect("the container exists")
        .value_view()
        .expect("the value views")
    else {
        panic!("Elements is a container");
    };

    assert_eq!(elements.item_kind(), PropertyKind::Embedded);
    assert_eq!(elements.len(), 4);

    let ValueView::Embedded(third) = elements
        .get(3)
        .expect("the item reads")
        .expect("there are four items")
    else {
        panic!("the items are embeds");
    };

    let position = third
        .property(POSITION)
        .expect("the walk runs")
        .expect("every embed has one")
        .value_view()
        .expect("the value views");
    assert!(matches!(position, ValueView::Vector2(v) if v == Vec2::new(3.0, -3.0)));

    assert!(elements.get(4).expect("the bound is checked").is_none());
}

/// A fixed-width item kind indexes by arithmetic and a variable-width one by walking. Both land
/// on the item the iterator yields.
#[test]
fn container_indexing_agrees_with_iteration() {
    let (bin, bytes) = every_kind_bin();
    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object.view().expect("the object views");

    let ValueView::Container(strings) = view
        .property(STRINGS)
        .expect("the walk runs")
        .expect("the container exists")
        .value_view()
        .expect("the value views")
    else {
        panic!("the strings are a container");
    };

    let walked: Vec<_> = strings
        .iter()
        .collect::<Result<_, _>>()
        .expect("every item reads");
    assert_eq!(walked.len(), 3);
    for (index, item) in walked.iter().enumerate() {
        let indexed = strings
            .get(index as u32)
            .expect("the item reads")
            .expect("the index is in range");
        assert!(matches!(
            (item, indexed),
            (ValueView::String(a), ValueView::String(b)) if *a == b
        ));
    }

    let ValueView::UnorderedContainer(numbers) = view
        .property(25u32)
        .expect("the walk runs")
        .expect("property 25 is the unordered container")
        .value_view()
        .expect("the value views")
    else {
        panic!("property 25 is an unordered container");
    };
    assert_eq!(numbers.item_kind(), PropertyKind::U8);
    assert!(matches!(
        numbers.get(0).expect("the item reads"),
        Some(ValueView::U8(9))
    ));
    assert!(numbers.get(1).expect("the bound is checked").is_none());
}

/// Descending is slice arithmetic: after the one read that buffers the object, nothing the
/// views can be asked for goes back to the source.
#[test]
fn a_view_stays_in_memory_once_the_object_is_buffered() {
    let (bin, bytes) = every_kind_bin();
    let object_hash = only_object(&bin);

    let (source, reads, _seeks) = Counting::new(io::Cursor::new(&bytes));
    let mut stream = BinStream::mount(source).expect("the stream mounts");
    stream.toc().expect("the TOC builds");

    let mut object = stream
        .object(object_hash)
        .expect("the TOC is cached")
        .expect("the object exists");
    let view = object.view().expect("the object views");
    let after = reads.get();

    // Everything the views can do, with the source watched.
    for property in view.properties() {
        let property = property.expect("the property reads");
        let _ = property.shape().expect("the shape reads");
        let _ = property.item_count().expect("the count reads");
        let _ = property.value().expect("the value decodes");
        if let ValueView::Container(list) = property.value_view().expect("the value views") {
            for item in list.iter() {
                let _ = item.expect("the item reads");
            }
        }
    }
    let _ = view.property(ELEMENTS).expect("the walk runs");
    let _ = view.raw();

    assert_eq!(
        reads.get(),
        after,
        "descending into the view touched the source"
    );
}

#[test]
fn a_views_raw_bytes_are_the_objects_whole_declared_range() {
    let (bin, bytes) = every_kind_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(only_object(&bin))
        .expect("the TOC builds")
        .expect("the object exists");
    let range = object.byte_range();
    let view = object.view().expect("the object views");

    assert_eq!(view.raw(), &bytes[range.start as usize..range.end as usize]);
}

// =============================================================================
// Owned decode
// =============================================================================

#[test]
fn into_bin_equals_what_the_bin_was_built_from() {
    let (bin, bytes) = every_kind_bin();
    let streamed = BinStream::mount(io::Cursor::new(&bytes))
        .expect("the stream mounts")
        .into_bin()
        .expect("the drain reads");

    assert_eq!(streamed, bin);
}

#[test]
fn reading_one_object_equals_the_eager_parse_of_it() {
    let (bin, bytes) = every_kind_bin();
    let eager = &bin.objects[&only_object(&bin)];

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut object = stream
        .object(eager.path_hash)
        .expect("the TOC builds")
        .expect("the object exists");

    assert_eq!(&object.read().expect("the object reads"), eager);
}

#[test]
fn a_size_the_counts_disagree_with_is_an_invalid_size_error() {
    let (bin, mut bytes) = every_kind_bin();
    let object_hash = only_object(&bin);

    let entry = {
        let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
        stream
            .object(object_hash)
            .expect("the TOC builds")
            .expect("the object exists")
            .entry()
    };

    // Claim four bytes more than the object has, and pad the file so the range still reads.
    let at = entry.offset as usize;
    bytes[at..at + 4].copy_from_slice(&(entry.size + 4).to_le_bytes());
    bytes.extend_from_slice(&[0xFF; 4]);

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let error = stream
        .object(object_hash)
        .expect("the TOC builds")
        .expect("the object exists")
        .view()
        .expect_err("the counts disagree with the declared size");
    assert!(
        matches!(error, Error::InvalidSize(declared, consumed)
            if declared == u64::from(entry.size) + 4 && consumed == u64::from(entry.size)),
        "unexpected error: {error}"
    );

    // The eager reader raises the same variant for the same bytes.
    let error = Bin::from_reader(&mut io::Cursor::new(&bytes))
        .expect_err("the counts disagree with the declared size");
    assert!(
        matches!(error, Error::InvalidSize(..)),
        "unexpected error: {error}"
    );
}

// =============================================================================
// The legacy-numbering latch
// =============================================================================

/// A bin whose one property is a null pointer written with the legacy kind byte for `Struct`.
///
/// Legacy numbering has no `WadChunkLink`, so the complex kinds sit lower than they do now:
/// `Struct` is `19`, which decodes as nothing at all under the current numbering.
fn legacy_numbered_bin() -> (BinHash, Vec<u8>) {
    let object = BinObject::builder(0x3333_0001u32, 0xCCCC_0001u32)
        .property(0x0001u32, values::Struct::default())
        .build();
    let bin = Bin::new([object], std::iter::empty::<&str>());

    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    let mut bytes = cursor.into_inner();

    let modern: u8 = PropertyKind::Struct.into();
    let at = bytes
        .iter()
        .position(|&byte| byte == modern)
        .expect("the kind byte is in there");
    bytes[at] = 19;

    (0x3333_0001u32.into(), bytes)
}

#[test]
fn a_legacy_numbered_file_latches_re_walks_and_reports_it() {
    let (object_hash, bytes) = legacy_numbered_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    assert_eq!(
        stream.numbering(),
        Numbering::Current,
        "mounting starts in the current numbering"
    );

    let mut object = stream
        .object(object_hash)
        .expect("the TOC builds")
        .expect("the object exists");
    let view = object
        .view()
        .expect("the legacy numbering explains the object");

    assert!(
        view.numbering().is_legacy(),
        "the view captured the latched numbering"
    );
    let property = view
        .properties()
        .next()
        .expect("one property")
        .expect("it reads");
    assert_eq!(property.kind(), PropertyKind::Struct);

    let _ = view;
    let _ = object;
    assert!(stream.numbering().is_legacy(), "the handle latched");

    // The drain reproduces the eager reader's whole-table retry.
    let bin = BinStream::mount(io::Cursor::new(&bytes))
        .expect("the stream mounts")
        .into_bin()
        .expect("the drain latches too");
    assert_eq!(
        bin.objects[&object_hash].properties[&BinHash(1)],
        values::Struct::default().into()
    );
}

#[test]
fn a_kind_byte_neither_numbering_explains_is_still_an_error() {
    let (_, mut bytes) = legacy_numbered_bin();
    let at = bytes
        .iter()
        .position(|&byte| byte == 19)
        .expect("the legacy kind byte is in there");
    // 200 decodes as nothing under either numbering.
    bytes[at] = 200;

    let error = BinStream::mount(io::Cursor::new(&bytes))
        .expect("the stream mounts")
        .into_bin()
        .expect_err("the kind byte decodes under neither numbering");
    assert!(
        matches!(error, Error::InvalidPropertyTypePrimitive(_)),
        "unexpected error: {error}"
    );
}

// =============================================================================
// The lookup cache
// =============================================================================

#[test]
fn a_cache_hit_costs_no_io_and_shares_the_object() {
    let (bin, bytes) = every_kind_bin();
    let object_hash = only_object(&bin);

    let (source, reads, _seeks) = Counting::new(io::Cursor::new(&bytes));
    let mut stream = BinStream::mount(source).expect("the stream mounts");
    stream.set_cache(Box::new(LruObjectCache::new(
        NonZeroUsize::new(4).expect("non-zero"),
    )));

    let first = stream
        .cached_object(object_hash)
        .expect("the lookup runs")
        .expect("the object exists");
    let after_miss = reads.get();

    let again = stream
        .cached_object(object_hash)
        .expect("the lookup runs")
        .expect("the object exists");

    assert_eq!(reads.get(), after_miss, "the hit went back to the source");
    assert!(Arc::ptr_eq(&first, &again), "the hit built a second object");
    assert_eq!(*first, bin.objects[&object_hash]);

    assert!(stream
        .cached_object(0xDEAD_BEEFu32)
        .expect("the lookup runs")
        .is_none());
}

#[test]
fn the_default_cache_parses_on_every_call() {
    let (bin, bytes) = every_kind_bin();
    let object_hash = only_object(&bin);

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let first = stream
        .cached_object(object_hash)
        .expect("the lookup runs")
        .expect("the object exists");
    let again = stream
        .cached_object(object_hash)
        .expect("the lookup runs")
        .expect("the object exists");

    assert!(!Arc::ptr_eq(&first, &again), "NoCache kept something");
    assert_eq!(first, again);
}

#[test]
fn a_handle_with_a_cache_installed_is_still_send() {
    fn assert_send<T: Send>() {}
    assert_send::<BinStream<io::Cursor<Vec<u8>>>>();
}

// =============================================================================
// Batch lookup
// =============================================================================

/// A five-object bin, and its path hashes in file order.
fn five_object_bin() -> (Vec<BinHash>, Vec<u8>) {
    let bin = Bin::new(
        (0..5u32).map(|index| {
            BinObject::builder(0x4444_0000u32 + index, 0xDDDD_0000u32 + index)
                .property(0x0001u32, values::U32::new(index))
                .build()
        }),
        std::iter::empty::<&str>(),
    );

    let mut cursor = io::Cursor::new(Vec::new());
    bin.to_writer(&mut cursor).expect("the bin writes");
    (bin.objects.keys().copied().collect(), cursor.into_inner())
}

#[test]
fn a_cold_batch_scans_once_and_stops_at_the_last_hit() {
    let (hashes, bytes) = five_object_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut batch = stream.objects_batch([hashes[1], hashes[0]]);

    let mut opened = Vec::new();
    while let Some(object) = batch.next().expect("the scan advances") {
        opened.push(object.path_hash());
    }
    assert!(batch.missing().is_empty());

    // File order, not request order.
    assert_eq!(opened, [hashes[0], hashes[1]]);

    // The scan stopped at the last hit: the rows past it were never harvested.
    assert!(!stream.is_toc_complete());
    assert!(stream.toc_row(2).is_none());
}

#[test]
fn a_warm_batch_visits_the_requested_rows_in_offset_order() {
    let (hashes, bytes) = five_object_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    stream.toc().expect("the TOC builds");

    // Requested back to front, with a duplicate and an absent hash mixed in.
    let mut batch = stream.objects_batch([
        hashes[4],
        hashes[2],
        BinHash(0xDEAD_BEEF),
        hashes[4],
        hashes[0],
    ]);

    let mut offsets = Vec::new();
    let mut opened = Vec::new();
    while let Some(object) = batch.next().expect("the cursor advances") {
        offsets.push(object.entry().offset);
        opened.push(object.path_hash());
    }

    assert_eq!(opened, [hashes[0], hashes[2], hashes[4]], "not file order");
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "the cursor seeked backwards: {offsets:?}"
    );
    assert_eq!(batch.missing(), [BinHash(0xDEAD_BEEF)]);
}

#[test]
fn a_batch_reports_what_the_table_does_not_hold() {
    let (hashes, bytes) = five_object_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let mut batch = stream.objects_batch([0x1u32, 0x2u32]);

    assert!(
        batch.missing().is_empty(),
        "a scan rules nothing out before it has run"
    );
    assert!(batch.next().expect("the scan runs").is_none());
    assert_eq!(batch.missing(), [BinHash(1), BinHash(2)]);

    // An empty request finds nothing, misses nothing, and reads nothing.
    let harvested = stream.toc_row(0).is_some();
    let mut batch = stream.objects_batch(Vec::<BinHash>::new());
    assert!(batch.next().expect("the scan runs").is_none());
    assert!(batch.missing().is_empty());
    assert_eq!(
        stream.toc_row(0).is_some(),
        harvested,
        "an empty request swept the table"
    );

    let mut batch = stream.objects_batch(hashes.clone());
    let mut seen = 0;
    while batch.next().expect("the cursor advances").is_some() {
        seen += 1;
    }
    assert_eq!(seen, hashes.len());
}

#[test]
fn a_batch_opens_the_same_objects_as_per_hash_lookups() {
    let (hashes, bytes) = five_object_bin();

    let mut stream = BinStream::mount(io::Cursor::new(&bytes)).expect("the stream mounts");
    let one_by_one: Vec<_> = hashes
        .iter()
        .map(|&hash| {
            stream
                .object(hash)
                .expect("the TOC builds")
                .expect("the object exists")
                .read()
                .expect("the object reads")
        })
        .collect();

    let mut batched = Vec::new();
    let mut batch = stream.objects_batch(hashes.iter().rev().copied());
    while let Some(mut object) = batch.next().expect("the cursor advances") {
        batched.push(object.read().expect("the object reads"));
    }

    assert_eq!(batched, one_by_one);
}
