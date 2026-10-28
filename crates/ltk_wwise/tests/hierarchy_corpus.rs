//! Every shipped `HIRC` of an installed client: framing and write back.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test hierarchy_corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use ltk_wwise::{bank::Section, hierarchy::ObjectKind, Bank, Error};

/// Returns the `(tag, body)` of each chunk of `bytes` in sequence.
fn chunks(bytes: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0;
    while at + 8 <= bytes.len() {
        let tag = bytes[at..at + 4].try_into().unwrap();
        let size = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
        let Some(body) = bytes.get(at + 8..at + 8 + size) else {
            break;
        };
        out.push((tag, body));
        at += 8 + size;
    }
    out
}

#[derive(Default)]
struct Counts {
    banks: usize,
    hierarchies: usize,
    written_back: usize,
    objects: usize,
    by_version: BTreeMap<u32, usize>,
    by_kind: BTreeMap<ObjectKind, usize>,
    skipped: BTreeMap<String, usize>,
}

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_hierarchy_frames_its_chunk_and_writes_back_byte_for_byte() {
    let mut counts = Counts::default();
    let mut failures = Vec::new();

    common::for_each_chunk(b"BKHD", |chunk| {
        counts.banks += 1;
        let bytes = &chunk.data[..];
        let where_ = format!("{} {:016x}", chunk.wad.display(), chunk.path_hash);
        let hirc_chunks = chunks(bytes)
            .iter()
            .filter(|(tag, _)| tag == b"HIRC")
            .count();

        let bank = match Bank::from_reader(&mut &bytes[..]) {
            Ok(bank) => bank,
            Err(error @ (Error::Framing { .. } | Error::PayloadTooShort(_))) => {
                failures.push(format!("{where_}: HIRC does not frame: {error}"));
                return;
            }
            Err(error) => {
                // `bank_corpus` accounts for the banks whose chunk stream does not read.
                let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
                let debug = format!("{error:?}");
                let variant = debug.split([' ', '(']).next().unwrap_or_default();
                let key = format!("v{version} {variant}");
                *counts.skipped.entry(key).or_default() += 1;
                return;
            }
        };

        let hierarchies: Vec<_> = bank
            .sections()
            .iter()
            .filter_map(|section| match section {
                Section::Hierarchy(hierarchy) => Some(hierarchy),
                _ => None,
            })
            .collect();
        if hierarchies.len() != hirc_chunks {
            failures.push(format!(
                "{where_}: {hirc_chunks} HIRC chunks, {} hierarchies",
                hierarchies.len()
            ));
        }
        if hierarchies.is_empty() {
            return;
        }

        let mut out = Vec::new();
        bank.to_writer(&mut out).unwrap();
        if out == bytes {
            counts.written_back += hierarchies.len();
        } else {
            failures.push(format!("{where_}: does not write back"));
        }

        let version = bank.header().version.0;
        for hierarchy in hierarchies {
            counts.hierarchies += 1;
            *counts.by_version.entry(version).or_default() += 1;
            counts.objects += hierarchy.len();
            for object in hierarchy.objects() {
                *counts.by_kind.entry(object.kind()).or_default() += 1;
            }
        }
    });

    println!("banks:              {}", counts.banks);
    println!("hierarchies:        {}", counts.hierarchies);
    println!("written back:       {}", counts.written_back);
    println!("by version:         {:?}", counts.by_version);
    println!("objects:            {}", counts.objects);
    println!("skipped (no read):  {:?}", counts.skipped);
    println!("objects by kind:");
    for (kind, count) in &counts.by_kind {
        println!("  {:>2} {:<32} {count}", kind.0, kind.to_string());
    }

    assert!(counts.hierarchies > 0, "no hierarchies found");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
