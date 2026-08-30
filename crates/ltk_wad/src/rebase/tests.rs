//! Tests for rebasing a WAD: the tail strategy and the layout it reads.

use super::*;
use crate::{Wad, WadBuilder, WadChunkBuilder, WadChunkCompression};
use ltk_hash::Hash as _;
use std::io::{Cursor, Write};

const SKIN: &str = "assets/characters/test/skins/skin0.dds";
const VFX: &str = "assets/characters/test/particles.bin";

/// The path hash a chunk at `path` would have.
fn hash(path: &str) -> WadHash {
    WadHash::hash_str(path)
}

/// Build an uncompressed WAD holding `chunks` as `(path, bytes)` pairs.
///
/// Uncompressed on purpose: a test that inspects the data region needs to
/// find the bytes it wrote.
///
/// # Panics
///
/// Panics when the fixture cannot be built, which means the fixture is wrong.
fn build_wad(chunks: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = WadBuilder::default();
    for (path, _) in chunks {
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_path(*path)
                .with_force_compression(WadChunkCompression::None),
        );
    }

    let by_hash: BTreeMap<WadHash, Vec<u8>> = chunks
        .iter()
        .map(|(path, bytes)| (hash(path), bytes.to_vec()))
        .collect();

    let mut cursor = Cursor::new(Vec::new());
    builder
        .build_to_writer(&mut cursor, move |chunk_hash, writer| {
            writer.write_all(&by_hash[&chunk_hash])?;
            Ok(())
        })
        .expect("fixture WAD builds");
    cursor.into_inner()
}

/// The layout of a WAD the builder just wrote, whose tail is still empty.
///
/// The builder puts the data directly behind a TOC sized to the chunk count,
/// which is the same shape a rebase reads. The region is read off the mounted
/// chunks rather than computed from the header size, so a layout here describes
/// where the bytes actually are: that is what lets
/// `a_built_wad_puts_its_toc_exactly_past_the_header` pin the constant against
/// a real build instead of against its own arithmetic.
///
/// # Panics
///
/// Panics when the bytes are not a WAD or hold no chunks, which means the
/// fixture is wrong.
fn layout_of(wad_bytes: &[u8]) -> (WadTailLayout, BTreeMap<WadHash, WadChunk>) {
    let wad = Wad::mount(Cursor::new(wad_bytes)).expect("the fixture mounts");
    let chunks = wad.chunks();
    let toc_capacity = u32::try_from(chunks.len()).expect("the fixture is small");

    let data_region_offset = chunks
        .iter()
        .map(|chunk| chunk.data_offset as u64)
        .min()
        .expect("the fixture holds chunks");
    let tail_offset = chunks
        .iter()
        .map(|chunk| (chunk.data_offset + chunk.compressed_size) as u64)
        .max()
        .expect("the fixture holds chunks");

    let layout = WadTailLayout {
        data_region_offset,
        // Zero: the fixture's own entries are already where the region puts
        // them. `a_layout_shifts_a_chunk_into_its_region` covers a real delta.
        offset_delta: 0,
        tail_offset,
        toc_capacity,
    };
    let entries = chunks
        .iter()
        .map(|chunk| (chunk.path_hash, layout.shifted(chunk).expect("in range")))
        .collect();

    (layout, entries)
}

/// An encoded chunk holding `data` verbatim, stored rather than compressed.
fn stored(data: &[u8]) -> EncodedChunk {
    EncodedChunk::new(
        data,
        u32::try_from(data.len()).expect("the fixture is small"),
        WadChunkCompression::None,
    )
}

/// Rewriting in place is destructive - the caller truncates the file before
/// the write - so a rewrite that is going to be refused must be refused
/// before that happens, or the fallback inherits a torn file it did not need
/// to. Planning is what refuses, and a plan never touches the WAD.
#[test]
fn a_rejected_entry_count_leaves_the_file_untouched() {
    let tmp = tempfile::tempdir().expect("a temp directory");
    let wad_path = tmp.path().join("Test.wad.client");

    let built = build_wad(&[(SKIN, b"the original skin"), (VFX, b"the original vfx")]);
    std::fs::write(&wad_path, &built).expect("the fixture writes");
    let (layout, base_entries) = layout_of(&built);

    let before = std::fs::read(&wad_path).expect("the fixture reads back");

    // One chunk the file reserved no TOC entry for, which is exactly what
    // the capacity check exists to refuse.
    let new_entry = hash("assets/characters/test/brand_new.bin");
    let tail = [(new_entry, stored(b"a chunk the TOC has no room for"))];
    let refused = WadRebasePlan::tail(&layout, base_entries, &tail);

    assert!(
        refused.is_err(),
        "an over-capacity entry set must be refused"
    );
    assert_eq!(
        std::fs::read(&wad_path).expect("the file reads back"),
        before,
        "a refused rewrite must not have touched the file"
    );
}

/// Story: the same rewrite, into a WAD that is not the whole file.
///
/// A WAD packed inside an archive starts partway through its container, so
/// every seek the write makes has to be offset by where it begins - and every
/// offset it *records* must not be, because the game reads the WAD without
/// knowing what holds it. Getting that backwards produces a WAD whose TOC
/// points at the container's bytes.
#[test]
fn a_tail_written_at_a_base_offset_reads_back_as_a_wad() {
    const BASE: u64 = 4096;

    let built = build_wad(&[(SKIN, b"the original skin"), (VFX, b"the original vfx")]);
    let (layout, base_entries) = layout_of(&built);

    // The container: padding, then the WAD cut back to where its tail begins -
    // which is the room a caller makes the same way.
    let mut container = vec![0xAAu8; BASE as usize];
    container.extend_from_slice(&built[..layout.tail_offset as usize]);

    let tail = [(hash(SKIN), stored(b"a differently modded skin"))];
    let plan = WadRebasePlan::tail(&layout, base_entries, &tail).expect("the plan is admissible");
    let tail_len = plan.tail_len();

    let mut cursor = Cursor::new(container);
    let report = plan.write(&mut cursor, BASE).expect("the tail writes");
    let container = cursor.into_inner();

    assert_eq!(
        report.tail_len, tail_len,
        "the plan predicted the tail length"
    );
    assert_eq!(report.entry_count, 2);

    // The padding is untouched: a write at a base offset stays inside the WAD.
    assert!(
        container[..BASE as usize].iter().all(|byte| *byte == 0xAA),
        "the write ran back over the container's own bytes"
    );

    // And the region reads as a WAD in its own right, holding the replacement.
    let mut wad = Wad::mount(Cursor::new(&container[BASE as usize..])).expect("the region mounts");
    let chunk = *wad.chunks().get(hash(SKIN)).expect("the rebased chunk");
    assert_eq!(
        &*wad
            .load_chunk_decompressed(&chunk)
            .expect("it decompresses"),
        b"a differently modded skin"
    );
    let untouched = *wad
        .chunks()
        .get(hash(VFX))
        .expect("the chunk the region still holds");
    assert_eq!(
        &*wad
            .load_chunk_decompressed(&untouched)
            .expect("it decompresses"),
        b"the original vfx"
    );
}

/// A layout puts the chunk count and the TOC in front of its data region by
/// subtraction, and a rebase seeks to that result and writes. A region offset
/// that leaves no room for the header means those writes land on the magic
/// and Riot's signature, so the layout must be refused before anything opens
/// the file.
#[test]
fn a_layout_whose_toc_would_land_in_the_header_is_refused() {
    // A v3.4 header is 268 bytes, so the smallest coherent region offset for
    // a one-entry TOC is 268 + 4 (the chunk count) + 32 (the entry).
    let smallest = WadTailLayout {
        data_region_offset: 268 + 4 + 32,
        offset_delta: 0,
        tail_offset: 4096,
        toc_capacity: 1,
    };
    assert!(
        smallest.validate().is_ok(),
        "the tightest legal layout must still be usable"
    );

    let in_the_header = WadTailLayout {
        data_region_offset: 4 + 32,
        ..smallest
    };
    assert!(
        in_the_header.validate().is_err(),
        "a region offset that leaves no room for the header must be refused"
    );
}

/// The header size `validate` reserves is the one the crate's own builder
/// emits. `layout_of` reads the region straight off the built WAD's chunks, so
/// this compares the constant against the bytes rather than against arithmetic
/// that started from the constant.
#[test]
fn a_built_wad_puts_its_toc_exactly_past_the_header() {
    let built = build_wad(&[(SKIN, b"the original skin")]);
    let (layout, _) = layout_of(&built);

    assert_eq!(
        layout.chunk_count_offset().expect("the layout is coherent"),
        268,
        "the chunk count follows the header"
    );
    assert_eq!(
        layout.toc_offset().expect("the layout is coherent"),
        272,
        "the TOC follows the chunk count"
    );
}

/// A tail hash the base entries already hold replaces one rather than adding
/// one, which is what lets a chunk whose replacement was removed revert to
/// the region's bytes without changing the WAD's shape.
#[test]
fn a_tail_hash_the_base_already_holds_adds_no_entry() {
    let built = build_wad(&[(SKIN, b"the original skin"), (VFX, b"the original vfx")]);
    let (_, base_entries) = layout_of(&built);

    assert_eq!(
        WadRebasePlan::merged_entry_count(&base_entries, [hash(SKIN)]),
        2,
        "overriding a chunk the region holds reuses its entry"
    );
    assert_eq!(
        WadRebasePlan::merged_entry_count(&base_entries, [hash("assets/new.bin")]),
        3,
        "a hash the region does not hold needs an entry of its own"
    );
}

/// A chunk copied into a rebasable WAD keeps every field but its offset, which
/// moves by the layout's delta - and the delta is signed, because the TOC the
/// chunk lands behind can be shorter than the one it came from. Past the 4 GiB
/// the format's `u32` offsets reach, the shift has to be refused rather than
/// truncated into an entry pointing at the wrong bytes.
#[test]
fn a_layout_shifts_a_chunk_into_its_region() {
    let source = WadChunk {
        path_hash: hash(SKIN),
        data_offset: 8192,
        compressed_size: 512,
        uncompressed_size: 1024,
        compression_type: WadChunkCompression::Zstd,
        is_duplicated: false,
        frame_count: 3,
        start_frame: 7,
        checksum: 0xDEAD_BEEF_CAFE_BABE,
    };
    let layout = WadTailLayout {
        data_region_offset: 4096,
        offset_delta: -4096,
        tail_offset: 65536,
        toc_capacity: 1,
    };

    let shifted = layout.shifted(&source).expect("the shift is addressable");
    assert_eq!(shifted.data_offset, 4096, "the offset moved by the delta");
    assert_eq!(
        WadChunk {
            data_offset: source.data_offset,
            ..shifted
        },
        source,
        "the offset is the only field a shift touches"
    );

    let past_the_limit = WadTailLayout {
        offset_delta: i64::from(u32::MAX),
        ..layout
    };
    assert!(
        past_the_limit.shifted(&source).is_err(),
        "a chunk shifted past the addressable file must be refused"
    );
}
