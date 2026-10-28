//! Every shipped version 132 and 134 bank: upgrade to version 145.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test upgrade_corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use ltk_wwise::{bank::BankVersion, Bank, Error, UpgradeRefusal};

/// The versions that the upgrade rewrites.
const LEGACY: [BankVersion; 2] = [BankVersion::V132, BankVersion::V134];

#[derive(Default)]
struct Counts {
    /// The legacy banks read, by version.
    banks: BTreeMap<u32, usize>,
    /// The banks that upgrade, by version.
    upgraded: BTreeMap<u32, usize>,
    /// The refused banks, by refusal.
    refused: BTreeMap<String, usize>,
    /// The upgraded banks with a media index.
    media_banks: usize,
    /// The upgraded media banks whose first `DATA` body starts at a multiple of 16.
    data_on_16: usize,
}

/// Returns the file offset of the first `DATA` body of the bank `bytes`.
fn data_body_offset(bytes: &[u8]) -> Option<usize> {
    let mut offset = 0;
    while let Some(head) = bytes.get(offset..offset + 8) {
        let size = u32::from_le_bytes(head[4..8].try_into().unwrap()) as usize;
        if &head[..4] == b"DATA" {
            return Some(offset + 8);
        }
        offset += 8 + size;
    }
    None
}

/// Returns why the upgrade of `bank` is not a checked version 145 bank whose objects decode.
fn verify(upgraded: &Bank, counts: &mut Counts) -> Result<(), String> {
    let mut bytes = Vec::new();
    upgraded
        .to_writer(&mut bytes)
        .map_err(|e| format!("write: {e}"))?;
    let reread = Bank::from_reader(&mut &bytes[..]).map_err(|e| format!("reread: {e}"))?;
    // A read media index and a built one with the same media compare unequal. The bytes
    // compare the content.
    let mut rewritten = Vec::new();
    reread
        .to_writer(&mut rewritten)
        .map_err(|e| format!("rewrite: {e}"))?;
    if rewritten != bytes {
        return Err("rereads to a bank that writes different bytes".to_owned());
    }
    reread.check().map_err(|r| format!("check: {r}"))?;
    if let Some(hierarchy) = reread.hierarchy() {
        for object in hierarchy.objects() {
            object
                .decode(BankVersion::V145)
                .map_err(|e| format!("decode {} {}: {e}", object.kind(), object.id()))?;
        }
    }
    if reread.media().is_some() {
        counts.media_banks += 1;
        if data_body_offset(&bytes).is_some_and(|offset| offset % 16 == 0) {
            counts.data_on_16 += 1;
        }
    }
    Ok(())
}

/// Returns the name of the class of `refusal` under which the test counts it.
fn refusal_key(refusal: UpgradeRefusal) -> String {
    match refusal {
        UpgradeRefusal::Kind { kind, .. } => format!("kind {}", kind.0),
        UpgradeRefusal::ActionClass { class, .. } => format!("action class {class:#06x}"),
        other => format!("{other:?}"),
    }
}

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_legacy_bank_without_a_refused_kind_upgrades_to_a_checked_bank() {
    let mut counts = Counts::default();
    let mut failures = Vec::new();

    common::for_each_chunk(b"BKHD", |chunk| {
        // `bank_corpus` accounts for the banks that do not read.
        let Ok(bank) = Bank::from_reader(&mut &chunk.data[..]) else {
            return;
        };
        let version = bank.header().version;
        if !LEGACY.contains(&version) {
            return;
        }
        *counts.banks.entry(version.0).or_default() += 1;
        let where_ = format!(
            "{} {:016x} v{version}",
            chunk.wad.display(),
            chunk.path_hash
        );
        match bank.upgrade() {
            Ok(upgraded) => match verify(&upgraded, &mut counts) {
                Ok(()) => *counts.upgraded.entry(version.0).or_default() += 1,
                Err(why) => failures.push(format!("{where_}: {why}")),
            },
            Err(Error::Upgrade(refusal)) => {
                *counts.refused.entry(refusal_key(refusal)).or_default() += 1;
            }
            Err(error) => failures.push(format!("{where_}: upgrade: {error}")),
        }
    });

    println!("legacy banks by version: {:?}", counts.banks);
    println!("upgraded by version: {:?}", counts.upgraded);
    println!("refused by reason: {:?}", counts.refused);
    println!(
        "upgraded media banks: {}, DATA body on a multiple of 16: {}",
        counts.media_banks, counts.data_on_16
    );
    for failure in &failures {
        println!("  {failure}");
    }

    assert!(!counts.banks.is_empty(), "no legacy banks found");
    assert!(
        failures.is_empty(),
        "{} failures, listed above",
        failures.len()
    );
}
