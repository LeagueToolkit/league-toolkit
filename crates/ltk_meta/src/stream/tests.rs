use std::{
    cell::Cell,
    io::{self, Read, Seek, SeekFrom},
    rc::Rc,
};

use crate::{concrete, concrete::values, Bin, BinKind, Error};

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
