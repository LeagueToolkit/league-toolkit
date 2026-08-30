//! What a rebase saves against a full rebuild, on real files.
//!
//! Run with `cargo run --release --example rebase_bench`.
//!
//! Both paths produce the same archive: one chunk's bytes replaced, every other
//! chunk unchanged. The rebuild path is the one the crate already had -
//! [`WadBuilder`] reading every chunk out of the source and writing it into a
//! new file. The rebase path rewrites the existing file's tail in place.
//!
//! The fixture is deliberately generous to the rebuild: its chunks are stored
//! uncompressed, so a rebuild is a pure byte copy with no codec to pay for,
//! which is the cheapest a full rebuild can possibly be. Both paths `sync_all`,
//! so the numbers include actually pushing the bytes to disk rather than just
//! handing them to the page cache.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use ltk_hash::Hash as _;
use ltk_wad::{
    EncodedChunk, Wad, WadBuilder, WadChunk, WadChunkBuilder, WadChunkCompression, WadHash,
    WadTailLayout,
};

/// Chunks in each fixture archive.
const CHUNK_COUNT: usize = 1024;

/// Archive sizes to compare, as the size of each of the `CHUNK_COUNT` chunks.
const CHUNK_SIZES: &[usize] = &[64 << 10, 256 << 10, 1 << 20];

fn main() {
    let tmp = tempfile::tempdir().expect("a temp directory");

    println!(
        "{:>10}  {:>12}  {:>12}  {:>10}  {:>14}  {:>14}",
        "archive", "rebuild", "rebase", "speedup", "rebuild wrote", "rebase wrote"
    );

    for &chunk_size in CHUNK_SIZES {
        let source = tmp.path().join(format!("source_{chunk_size}.wad.client"));
        write_fixture(&source, chunk_size);

        let replacement = vec![0xCDu8; chunk_size];
        let changed = WadHash::hash_str("assets/chunk_0.bin");

        let rebuilt = tmp.path().join(format!("rebuilt_{chunk_size}.wad.client"));
        let (rebuild_time, rebuild_bytes) = time_rebuild(&source, &rebuilt, changed, &replacement);

        // The rebase rewrites an archive that already exists, so the copy is
        // setup and not part of the measurement. It is synced here rather than
        // left dirty, or the rebase's own `sync_all` would be billed for
        // flushing the whole copy and the measurement would scale with the
        // archive instead of with the change.
        let rebased = tmp.path().join(format!("rebased_{chunk_size}.wad.client"));
        std::fs::copy(&source, &rebased).expect("the overlay copy is made");
        OpenOptions::new()
            .write(true)
            .open(&rebased)
            .expect("the copy opens")
            .sync_all()
            .expect("the copy reaches the disk");
        let (rebase_time, rebase_bytes) = time_rebase(&rebased, changed, &replacement);

        assert_eq!(
            std::fs::read(&rebuilt)
                .expect("the rebuild reads back")
                .len(),
            rebuild_bytes as usize,
            "the rebuild wrote the whole archive"
        );
        check(&rebased, changed, &replacement);

        println!(
            "{:>10}  {:>12}  {:>12}  {:>9.0}x  {:>14}  {:>14}",
            mib(rebuild_bytes),
            millis(rebuild_time),
            millis(rebase_time),
            rebuild_time.as_secs_f64() / rebase_time.as_secs_f64(),
            mib(rebuild_bytes),
            kib(rebase_bytes),
        );

        // Three archives of every size are on disk at once; the next size is
        // four times bigger, so they do not all get to stay.
        for path in [&source, &rebuilt, &rebased] {
            std::fs::remove_file(path).expect("the fixture is removable");
        }
    }
}

/// Write an archive of `CHUNK_COUNT` stored chunks of `chunk_size` bytes each.
fn write_fixture(path: &Path, chunk_size: usize) {
    let mut builder = WadBuilder::default();
    for index in 0..CHUNK_COUNT {
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_path(format!("assets/chunk_{index}.bin"))
                .with_force_compression(WadChunkCompression::None),
        );
    }

    let payload = vec![0xABu8; chunk_size];
    let mut file = File::create(path).expect("the fixture is creatable");
    builder
        .build_to_writer(&mut file, |_hash, cursor| {
            cursor.write_all(&payload)?;
            Ok(())
        })
        .expect("the fixture builds");
    file.sync_all().expect("the fixture reaches the disk");
}

/// Rebuild the whole archive into a new file, with one chunk's bytes replaced.
///
/// Every chunk is read out of the source and written into the output, which is
/// what makes the cost the archive's size rather than the change's.
fn time_rebuild(
    source: &Path,
    output: &Path,
    changed: WadHash,
    replacement: &[u8],
) -> (Duration, u64) {
    let start = Instant::now();

    let wad = RefCell::new(mount(source));
    let mut builder = WadBuilder::default();
    for chunk in wad.borrow().chunks().iter() {
        builder = builder.with_chunk(
            WadChunkBuilder::default()
                .with_hash(chunk.path_hash)
                .with_force_compression(WadChunkCompression::None),
        );
    }

    let mut file = File::create(output).expect("the output is creatable");
    builder
        .build_to_writer(&mut file, |path_hash, cursor| {
            if path_hash == changed {
                cursor.write_all(replacement)?;
                return Ok(());
            }
            let mut wad = wad.borrow_mut();
            let chunk = *wad
                .chunks()
                .get(path_hash)
                .expect("the chunk is in the TOC");
            let raw = wad.load_chunk_raw(&chunk)?;
            cursor.write_all(&raw)?;
            Ok(())
        })
        .expect("the rebuild succeeds");
    file.sync_all().expect("the rebuild reaches the disk");

    let elapsed = start.elapsed();
    let written = file.metadata().expect("the output is measurable").len();
    (elapsed, written)
}

/// Rewrite one chunk into the archive's tail, in place.
fn time_rebase(path: &Path, changed: WadHash, replacement: &[u8]) -> (Duration, u64) {
    let start = Instant::now();

    let (layout, base_entries) = layout_of(&mount(path));
    let tail = [(
        changed,
        EncodedChunk::new(
            replacement,
            u32::try_from(replacement.len()).expect("the chunk fits"),
            WadChunkCompression::None,
        ),
    )];

    let plan =
        ltk_wad::WadRebasePlan::tail(&layout, base_entries, &tail).expect("it is admissible");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("the archive opens");
    file.set_len(layout.tail_offset).expect("it truncates");
    let report = plan.write(&mut file, 0).expect("the tail writes");
    file.sync_all().expect("the rebase reaches the disk");

    let elapsed = start.elapsed();
    // The tail, plus the chunk count and the TOC written over the top of it.
    let toc = 4 + u64::from(layout.toc_capacity) * 32;
    (elapsed, report.tail_len + toc)
}

/// The layout of an archive whose chunks sit directly behind its TOC.
fn layout_of<S: std::io::Read + std::io::Seek>(
    wad: &Wad<S>,
) -> (WadTailLayout, BTreeMap<WadHash, WadChunk>) {
    let chunks = wad.chunks();
    let data_region_offset = chunks
        .iter()
        .map(|chunk| chunk.data_offset as u64)
        .min()
        .expect("the archive holds chunks");
    let tail_offset = chunks
        .iter()
        .map(|chunk| (chunk.data_offset + chunk.compressed_size) as u64)
        .max()
        .expect("the archive holds chunks");

    let layout = WadTailLayout {
        data_region_offset,
        offset_delta: 0,
        tail_offset,
        toc_capacity: u32::try_from(chunks.len()).expect("the archive is addressable"),
    };
    let entries = chunks
        .iter()
        .map(|chunk| (chunk.path_hash, *chunk))
        .collect();
    (layout, entries)
}

/// Check the rebased archive reads back with the replacement in it.
fn check(path: &Path, changed: WadHash, replacement: &[u8]) {
    let mut wad = mount(path);
    let chunk = *wad.chunks().get(changed).expect("the rebased chunk");
    let read = wad
        .load_chunk_decompressed(&chunk)
        .expect("the rebased chunk decompresses");
    assert_eq!(&*read, replacement, "the rebase did not take");
}

fn mount(path: &Path) -> Wad<BufReader<File>> {
    let file = File::open(path).expect("the archive opens");
    Wad::mount(BufReader::new(file)).expect("the archive mounts")
}

fn millis(duration: Duration) -> String {
    format!("{:.1} ms", duration.as_secs_f64() * 1000.0)
}

fn mib(bytes: u64) -> String {
    format!("{:.0} MiB", bytes as f64 / (1024.0 * 1024.0))
}

fn kib(bytes: u64) -> String {
    format!("{:.0} KiB", bytes as f64 / 1024.0)
}
