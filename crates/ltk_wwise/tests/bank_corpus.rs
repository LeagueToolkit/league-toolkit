//! Every shipped bank of an installed client: read, write back, media order and acceptance.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test bank_corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use ltk_wwise::{
    bank::{Rejection, Section},
    Bank, Error,
};

/// Returns the `(source ID, offset)` records of a `DIDX` body.
fn records(didx: &[u8]) -> Vec<(u32, u32)> {
    let field = |r: &[u8], at: usize| u32::from_le_bytes([r[at], r[at + 1], r[at + 2], r[at + 3]]);
    didx.as_chunks::<12>()
        .0
        .iter()
        .map(|r| (field(r, 0), field(r, 4)))
        .collect()
}

/// Returns the `(tag, file offset of the body, body)` of each chunk of `bytes` in sequence.
fn chunks(bytes: &[u8]) -> Vec<([u8; 4], usize, &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let tag = bytes[at..at + 4].try_into().unwrap();
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let Some(body) = bytes.get(at + 8..at + 8 + size) else {
            break;
        };
        out.push((tag, at + 8, body));
        at += 8 + size;
    }
    out
}

#[derive(Default)]
struct Counts {
    banks: usize,
    read: usize,
    written_back: usize,
    read_errors: BTreeMap<String, usize>,
    versions: BTreeMap<u32, usize>,
    header_lens: BTreeMap<(u32, usize), usize>,
    media_indexes: usize,
    media: usize,
    data_body_aligned: usize,
    media_offsets_aligned: usize,
    legacy_with_other_chunk: usize,
    rejections: BTreeMap<String, usize>,
}

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_bank_writes_back_byte_for_byte() {
    let mut counts = Counts::default();
    let mut failures = Vec::new();

    common::for_each_chunk(b"BKHD", |chunk| {
        counts.banks += 1;
        let bytes = &chunk.data[..];
        let where_ = format!("{} {:016x}", chunk.wad.display(), chunk.path_hash);

        let bank = match Bank::from_reader(&mut &bytes[..]) {
            Ok(bank) => bank,
            Err(error) => {
                let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
                let kind = match error {
                    Error::ChunkOverrun { tag, .. } => {
                        format!("v{version} ChunkOverrun {}", tag.escape_ascii())
                    }
                    other => format!("v{version} {other:?}"),
                };
                // The shipped version 125 banks carry bytes between or after their chunks. The
                // client reads them in sequence and fails on them too.
                if version != 125 || !kind.contains("ChunkOverrun") {
                    failures.push(format!("{where_}: does not read: {kind}"));
                }
                *counts.read_errors.entry(kind).or_default() += 1;
                return;
            }
        };
        counts.read += 1;
        let header = bank.header();
        *counts.versions.entry(header.version.0).or_default() += 1;
        let header_len = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        *counts
            .header_lens
            .entry((header.version.0, header_len))
            .or_default() += 1;

        let mut out = Vec::new();
        bank.to_writer(&mut out).unwrap();
        if out == bytes {
            counts.written_back += 1;
        } else {
            let at = out.iter().zip(bytes).position(|(a, b)| a != b);
            let layout: Vec<_> = chunks(bytes)
                .iter()
                .map(|(tag, at, body)| format!("{}@{at}+{}", tag.escape_ascii(), body.len()))
                .collect();
            failures.push(format!(
                "{where_}: does not write back: v{} {} bytes, wrote {}, first difference {at:?}, \
                 chunks {layout:?}",
                header.version,
                bytes.len(),
                out.len()
            ));
        }

        let alignment = header.media_alignment();
        let raw = chunks(bytes);
        for (i, (tag, _, didx)) in raw.iter().enumerate() {
            if tag != b"DIDX" {
                continue;
            }
            let Some(&(next_tag, data_at, _)) = raw.get(i + 1) else {
                continue;
            };
            if &next_tag != b"DATA" {
                continue;
            }
            counts.media_indexes += 1;
            let records = records(didx);
            counts.media += records.len();
            if !records.windows(2).all(|w| w[0].0 < w[1].0) {
                failures.push(format!(
                    "{where_}: DIDX is not in ascending source ID order"
                ));
            }
            if data_at % 16 == 0 {
                counts.data_body_aligned += 1;
            }
            if records.iter().all(|&(_, offset)| offset % alignment == 0) {
                counts.media_offsets_aligned += 1;
            }
        }

        let other_chunk = bank.sections().iter().any(|section| match section {
            Section::Raw(raw) => &raw.tag != b"DIDX" && &raw.tag != b"DATA",
            Section::Hierarchy(_) => true,
            Section::Media(_) => false,
        });
        if header.version.is_legacy() && other_chunk {
            counts.legacy_with_other_chunk += 1;
            if !matches!(bank.check(), Err(Rejection::LegacyChunk { .. })) {
                failures.push(format!("{where_}: legacy bank with a chunk passes check"));
            }
        }
        if let Err(rejection) = bank.check() {
            let key = match rejection {
                Rejection::LegacyChunk { version, tag } => {
                    format!("LegacyChunk v{version} {}", tag.escape_ascii())
                }
                other => format!("{other:?}"),
            };
            *counts.rejections.entry(key).or_default() += 1;
        }
    });

    println!("banks:                    {}", counts.banks);
    println!("read:                     {}", counts.read);
    println!("written back:             {}", counts.written_back);
    println!("read errors:              {:?}", counts.read_errors);
    println!("versions:                 {:?}", counts.versions);
    println!("BKHD sizes (version, len): {:?}", counts.header_lens);
    println!("media indexes:            {}", counts.media_indexes);
    println!("media:                    {}", counts.media);
    println!("DATA body at 16n:         {}", counts.data_body_aligned);
    println!("media offsets aligned:    {}", counts.media_offsets_aligned);
    println!(
        "legacy with other chunk:  {}",
        counts.legacy_with_other_chunk
    );
    println!("rejections:               {:?}", counts.rejections);

    assert!(counts.banks > 0, "no banks found");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
