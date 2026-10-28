//! Every shipped object of a typed kind: decode and encode back to its payload.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test typed_corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use ltk_wwise::{
    bank::{BankVersion, Section},
    hierarchy::ObjectKind,
    Bank,
};

/// The bank versions with typed layouts.
const VERSIONS: [BankVersion; 3] = [BankVersion::V132, BankVersion::V134, BankVersion::V145];

/// The kinds with a typed layout.
const TYPED_KINDS: [ObjectKind; 7] = [
    ObjectKind::SOUND,
    ObjectKind::ACTION,
    ObjectKind::EVENT,
    ObjectKind::RANDOM_SEQUENCE_CONTAINER,
    ObjectKind::SWITCH_CONTAINER,
    ObjectKind::ACTOR_MIXER,
    ObjectKind::LAYER_CONTAINER,
];

#[derive(Default)]
struct Counts {
    banks: BTreeMap<u32, usize>,
    /// The objects that decode and encode back, by version and kind.
    round_trips: BTreeMap<(u32, ObjectKind), usize>,
    /// The objects that fail, by version and kind.
    failed: BTreeMap<(u32, ObjectKind), usize>,
}

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_object_of_a_typed_kind_decodes_and_encodes_back_to_its_payload() {
    let mut counts = Counts::default();
    let mut failures = Vec::new();

    common::for_each_chunk(b"BKHD", |chunk| {
        // `bank_corpus` accounts for the banks that do not read.
        let Ok(bank) = Bank::from_reader(&mut &chunk.data[..]) else {
            return;
        };
        let version = bank.header().version;
        if !VERSIONS.contains(&version) {
            return;
        }
        let where_ = format!("{} {:016x}", chunk.wad.display(), chunk.path_hash);
        let mut has_hierarchy = false;
        for section in bank.sections() {
            let Section::Hierarchy(hierarchy) = section else {
                continue;
            };
            has_hierarchy = true;
            for object in hierarchy.objects() {
                if !TYPED_KINDS.contains(&object.kind()) {
                    continue;
                }
                let key = (version.0, object.kind());
                let result = match object.decode(version) {
                    Ok(Some(typed)) => match typed.encode(version) {
                        Ok(encoded) if &encoded == object => Ok(()),
                        Ok(encoded) => Err(format!(
                            "encodes to {} bytes, not {}",
                            encoded.payload().len(),
                            object.payload().len()
                        )),
                        Err(error) => Err(format!("encode: {error}")),
                    },
                    Ok(None) => Err("no typed layout".to_owned()),
                    Err(error) => Err(format!("decode: {error}")),
                };
                match result {
                    Ok(()) => *counts.round_trips.entry(key).or_default() += 1,
                    Err(why) => {
                        *counts.failed.entry(key).or_default() += 1;
                        failures.push(format!(
                            "{where_} v{} {} {}: {why}",
                            version,
                            object.kind(),
                            object.id()
                        ));
                    }
                }
            }
        }
        if has_hierarchy {
            *counts.banks.entry(version.0).or_default() += 1;
        }
    });

    println!("banks with a hierarchy by version: {:?}", counts.banks);
    println!("round trips by version and kind:");
    for ((version, kind), count) in &counts.round_trips {
        println!(
            "  v{version} {:>2} {:<28} {count}",
            kind.0,
            kind.to_string()
        );
    }
    println!("failures by version and kind: {:?}", counts.failed);
    for failure in &failures {
        println!("  {failure}");
    }

    assert!(!counts.round_trips.is_empty(), "no typed objects found");
    assert!(
        failures.is_empty(),
        "{} failures, listed above",
        failures.len()
    );
}
