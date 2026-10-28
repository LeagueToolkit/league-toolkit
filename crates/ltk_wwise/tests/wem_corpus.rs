//! Reads the `WemInfo` of every media of every shipped package.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test wem_corpus -- --ignored --nocapture`.

mod common;

use std::collections::BTreeMap;

use ltk_wwise::{wem::WemInfo, Package};

/// The magic and the version of a package.
const PACKAGE_MAGIC: &[u8] = b"r3d2\x01\0\0\0";

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_wem_has_a_format() {
    let mut packages = 0usize;
    let mut media = 0usize;
    let mut codecs: BTreeMap<u16, usize> = BTreeMap::new();
    let mut channels: BTreeMap<u16, usize> = BTreeMap::new();
    let mut failures = Vec::new();

    common::for_each_chunk(PACKAGE_MAGIC, |chunk| {
        let where_ = format!("{} {:016x}", chunk.wad.display(), chunk.path_hash);
        packages += 1;
        let package = match Package::from_reader(&mut &chunk.data[..]) {
            Ok(package) => package,
            Err(error) => {
                failures.push(format!("{where_}: read: {error}"));
                return;
            }
        };
        for entry in package.entries() {
            media += 1;
            match WemInfo::from_bytes(entry.data()) {
                Ok(info) => {
                    *codecs.entry(info.codec.0).or_default() += 1;
                    *channels.entry(info.channels).or_default() += 1;
                }
                Err(error) => failures.push(format!("{where_} {}: {error}", entry.name())),
            }
        }
    });

    println!("{packages} packages, {media} media");
    for (codec, count) in &codecs {
        println!("codec {codec:#06x}: {count}");
    }
    for (channels, count) in &channels {
        println!("channels {channels}: {count}");
    }
    for failure in failures.iter().take(20) {
        println!("FAIL {failure}");
    }
    assert!(packages > 0, "no packages under LTK_LOL_GAME_DIR");
    assert!(failures.is_empty(), "{} failures", failures.len());
}
