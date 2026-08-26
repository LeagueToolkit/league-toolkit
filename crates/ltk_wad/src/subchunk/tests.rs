//! Tests for the subchunk table: parsing, detection, and partial reads.

use super::*;

fn record(compressed_size: u32, uncompressed_size: u32) -> WadSubchunk {
    WadSubchunk {
        compressed_size,
        uncompressed_size,
        checksum: 0,
    }
}

fn toc_bytes(records: &[WadSubchunk]) -> Vec<u8> {
    let mut data = Vec::new();
    for sub in records {
        data.extend_from_slice(&sub.compressed_size.to_le_bytes());
        data.extend_from_slice(&sub.uncompressed_size.to_le_bytes());
        data.extend_from_slice(&sub.checksum.to_le_bytes());
    }
    data
}

fn multi_chunk(start_frame: u32, frame_count: u8, sizes: (usize, usize)) -> WadChunk {
    WadChunk {
        path_hash: crate::WadHash(start_frame as u64 + 1),
        data_offset: 0,
        compressed_size: sizes.0,
        uncompressed_size: sizes.1,
        compression_type: WadChunkCompression::ZstdMulti,
        is_duplicated: false,
        frame_count,
        start_frame,
        checksum: 0,
    }
}

#[test]
fn parses_records_and_rejects_odd_lengths() {
    let records = [record(10, 20), record(5, 5)];
    let toc = SubchunkToc::from_bytes(&toc_bytes(&records)).unwrap();
    assert_eq!(toc.records(), records);
    assert!(toc.records()[1].is_raw());
    SubchunkToc::from_bytes(&[0u8; 17]).unwrap_err();
}

#[test]
fn subchunks_of_slices_by_frame_and_bounds() {
    let toc =
        SubchunkToc::from_bytes(&toc_bytes(&[record(1, 2), record(3, 4), record(5, 6)])).unwrap();
    let chunk = multi_chunk(1, 2, (8, 10));
    assert_eq!(
        toc.subchunks_of(&chunk),
        Some(&[record(3, 4), record(5, 6)][..])
    );
    assert_eq!(toc.subchunks_of(&multi_chunk(2, 2, (0, 0))), None);
    let mut zstd = chunk;
    zstd.compression_type = WadChunkCompression::Zstd;
    assert_eq!(toc.subchunks_of(&zstd), None);
}

#[test]
fn covers_wants_the_sums_to_match() {
    let toc = SubchunkToc::from_bytes(&toc_bytes(&[record(1, 2), record(3, 4)])).unwrap();
    let good = WadChunks::from_iter([multi_chunk(0, 2, (4, 6))]);
    assert!(toc.covers(&good));
    let bad = WadChunks::from_iter([multi_chunk(0, 2, (4, 7))]);
    assert!(!toc.covers(&bad));
    let out_of_range = WadChunks::from_iter([multi_chunk(1, 2, (8, 10))]);
    assert!(!toc.covers(&out_of_range));
}

/// One hardcoded frame, so this decodes under either zstd backend.
#[test]
fn subchunked_decode_runs_on_either_backend() {
    let content: Vec<u8> = b"league subchunk ".repeat(4);
    let frame: &[u8] = &[
        0x28, 0xb5, 0x2f, 0xfd, 0x00, 0x68, 0xc5, 0x00, 0x00, 0x88, 0x6c, 0x65, 0x61, 0x67, 0x75,
        0x65, 0x20, 0x73, 0x75, 0x62, 0x63, 0x68, 0x75, 0x6e, 0x6b, 0x20, 0x6c, 0x01, 0x00, 0x99,
        0x68, 0x2e, 0x01,
    ];
    let raw_run = b"raw (\xb5/\xfda fake frame start";
    let subchunks = [
        record(raw_run.len() as u32, raw_run.len() as u32),
        record(frame.len() as u32, content.len() as u32),
    ];
    let raw: Vec<u8> = [raw_run.as_slice(), frame].concat();
    let expected: Vec<u8> = [raw_run.as_slice(), &content].concat();

    let data = crate::decompress_subchunked(&raw, &subchunks, expected.len()).unwrap();
    assert_eq!(&data[..], &expected[..]);

    let kept = crate::ChunkDecoder::new()
        .decompress_subchunked(&raw, &subchunks, expected.len())
        .unwrap();
    assert_eq!(&kept[..], &expected[..]);

    let head = crate::ChunkDecoder::new()
        .decompress_subchunked_prefix(&raw, &subchunks, expected.len())
        .unwrap();
    assert_eq!(&head[..], &expected[..]);
}

/// A mountable wad: one subchunked chunk whose raw first subchunk holds
/// the zstd magic, and the table chunk, with real record checksums.
///
/// Returns the wad's bytes, its chunks, and the two runs the subchunked
/// chunk decodes to.
#[cfg(feature = "zstd")]
fn subchunked_wad() -> (Vec<u8>, [WadChunk; 2], Vec<u8>, Vec<u8>) {
    use byteorder::{WriteBytesExt as _, LE};
    use std::io::Cursor;
    use xxhash_rust::xxh3::xxh3_64;

    let raw_run = b"raw (\xb5/\xfda fake frame start".to_vec();
    let content = b"compressed content".repeat(20);
    let content_frame =
        zstd::encode_all(Cursor::new(&content[..]), 3).expect("encoding cannot fail");
    let chunk_data: Vec<u8> = [raw_run.as_slice(), &content_frame].concat();

    let mut records = [
        record(raw_run.len() as u32, raw_run.len() as u32),
        record(content_frame.len() as u32, content.len() as u32),
    ];
    records[0].checksum = xxh3_64(&raw_run);
    records[1].checksum = xxh3_64(&content_frame);
    let toc_data = toc_bytes(&records);

    /* Header, two 32-byte chunk records, then the data. */
    let data_start = 2 + 1 + 1 + 256 + 8 + 4 + 2 * 32;
    let chunks = [
        WadChunk {
            path_hash: crate::WadHash(1),
            data_offset: data_start,
            compressed_size: chunk_data.len(),
            uncompressed_size: raw_run.len() + content.len(),
            compression_type: WadChunkCompression::ZstdMulti,
            is_duplicated: false,
            frame_count: 2,
            start_frame: 0,
            checksum: 0,
        },
        WadChunk {
            path_hash: crate::WadHash(2),
            data_offset: data_start + chunk_data.len(),
            compressed_size: toc_data.len(),
            uncompressed_size: toc_data.len(),
            compression_type: WadChunkCompression::None,
            is_duplicated: false,
            frame_count: 0,
            start_frame: 0,
            checksum: 0,
        },
    ];

    let mut file = Vec::new();
    file.write_u16::<LE>(0x5752).unwrap();
    file.push(3);
    file.push(4);
    file.extend_from_slice(&[0u8; 256]);
    file.write_u64::<LE>(0).unwrap();
    file.write_i32::<LE>(chunks.len() as i32).unwrap();
    for chunk in &chunks {
        chunk.write_v3_4(&mut file).unwrap();
    }
    assert_eq!(file.len(), data_start);
    file.extend_from_slice(&chunk_data);
    file.extend_from_slice(&toc_data);

    (file, chunks, raw_run, content)
}

/// Mounting finds the table by shape and decodes a chunk whose raw first
/// subchunk holds the zstd magic, which the frame scan cannot.
#[test]
#[cfg(feature = "zstd")]
fn mount_detects_the_table_and_decodes_by_it() {
    use crate::Wad;
    use std::io::Cursor;

    let (file, mut chunks, raw_run, content) = subchunked_wad();
    let expected: Vec<u8> = [raw_run.as_slice(), &content].concat();

    let mut wad = Wad::mount(Cursor::new(file)).unwrap();
    let toc = wad.subchunk_toc().expect("the table is found by shape");
    assert_eq!(toc.records().len(), 2);

    let data = wad.load_chunk_decompressed(&chunks[0]).unwrap();
    assert_eq!(&data[..], &expected[..]);

    /* Without the table's chunk, mounting finds nothing and the scan
    fallback stands. */
    chunks[1].uncompressed_size += 16;
    let no_toc = detect_subchunk_toc(&WadChunks::from_iter(chunks), |_| {
        Err(WadError::Other(String::from("not read")))
    });
    assert!(no_toc.is_none());
}

/// The partial read: the first subchunk alone, then all of them, with the
/// record checksums standing guard.
#[test]
#[cfg(feature = "zstd")]
fn load_subchunks_reads_a_verified_prefix() {
    use crate::Wad;
    use std::io::Cursor;

    let (file, chunks, raw_run, content) = subchunked_wad();
    let chunk = chunks[0];

    let mut wad = Wad::mount(Cursor::new(file.clone())).unwrap();
    let first = wad.load_subchunks(&chunk, 1).unwrap();
    assert_eq!(&first[..], &raw_run[..]);

    let all = wad.load_subchunks(&chunk, 2).unwrap();
    assert_eq!(&all[..], &[raw_run.as_slice(), &content].concat()[..]);

    wad.load_subchunks(&chunk, 0).unwrap_err();
    wad.load_subchunks(&chunk, 3).unwrap_err();
    /* A chunk with no records in the table. */
    wad.load_subchunks(&chunks[1], 1).unwrap_err();

    /* One flipped bit in the first subchunk fails its checksum. */
    let mut corrupt = file;
    corrupt[chunk.data_offset] ^= 1;
    let mut wad = Wad::mount(Cursor::new(corrupt)).unwrap();
    let error = wad.load_subchunks(&chunk, 1).unwrap_err();
    assert!(error.to_string().contains("checksum"), "{error}");
}
