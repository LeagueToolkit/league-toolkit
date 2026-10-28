//! Reads the `WemInfo` of every media of every shipped package.
//!
//! Run with `LTK_LOL_GAME_DIR` set:
//! `cargo test -p ltk_wwise --release --test wem_corpus -- --ignored --nocapture`.

mod common;
mod wpk_media;

use std::collections::BTreeMap;

use ltk_wwise::wem::WemInfo;

#[test]
#[ignore = "reads the installed client named by LTK_LOL_GAME_DIR"]
fn every_shipped_wem_has_a_format() {
    let mut packages = 0usize;
    let mut media = 0usize;
    let mut codecs: BTreeMap<u16, usize> = BTreeMap::new();
    let mut channels: BTreeMap<u16, usize> = BTreeMap::new();
    let mut failures = Vec::new();

    common::for_each_chunk(b"r3d2\x01\0\0\0", |chunk| {
        let Some(entries) = wpk_media::entries(&chunk.data) else {
            failures.push(format!(
                "{} {:016x}: entry table out of bounds",
                chunk.wad.display(),
                chunk.path_hash
            ));
            return;
        };
        packages += 1;
        for (name, wem) in entries {
            media += 1;
            match WemInfo::from_bytes(wem) {
                Ok(info) => {
                    *codecs.entry(info.codec.0).or_default() += 1;
                    *channels.entry(info.channels).or_default() += 1;
                }
                Err(error) => failures.push(format!(
                    "{} {:016x} {name}: {error}",
                    chunk.wad.display(),
                    chunk.path_hash
                )),
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
    assert!(failures.is_empty(), "{} media failed", failures.len());
}
