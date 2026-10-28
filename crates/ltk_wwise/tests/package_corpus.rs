//! Writes every shipped package back, ignored unless you point it at an installed client.
//!
//! ```text
//! LTK_LOL_GAME_DIR="C:/Riot Games/League of Legends/Game" \
//!     cargo test -p ltk_wwise --test package_corpus --release -- --ignored --nocapture
//! ```
//!
//! Every package must write back byte for byte, and every entry key must equal the source ID of
//! its name. The animations `r3d2anmd` and `r3d2canm` share the magic. The walk matches the magic
//! and version 1.

mod common;

use std::io::Cursor;

use ltk_wwise::{package::PackageKey, MountedPackage, Package};

/// The magic and the version of a package.
const PACKAGE_MAGIC: &[u8] = b"r3d2\x01\0\0\0";

#[test]
#[ignore = "needs an installed client; set LTK_LOL_GAME_DIR"]
fn shipped_packages_write_back() {
    let (mut packages, mut empty, mut entries, mut bytes) = (0usize, 0usize, 0usize, 0usize);
    let (mut not_riff_wave, mut failures) = (0usize, Vec::new());

    common::for_each_chunk(PACKAGE_MAGIC, |chunk| {
        let name = format!(
            "{} {:016x}",
            chunk.wad.file_name().unwrap_or_default().to_string_lossy(),
            chunk.path_hash
        );
        packages += 1;
        bytes += chunk.data.len();

        let package = match Package::from_reader(&mut &chunk.data[..]) {
            Ok(package) => package,
            Err(e) => {
                failures.push(format!("{name}: read: {e}"));
                return;
            }
        };
        if package.is_empty() {
            empty += 1;
        }
        entries += package.len();

        let mut written = Vec::with_capacity(chunk.data.len());
        package.to_writer(&mut written).unwrap();
        if written[..] != chunk.data[..] {
            failures.push(format!(
                "{name}: writes {} bytes, shipped {}, first difference at {:?}",
                written.len(),
                chunk.data.len(),
                written
                    .iter()
                    .zip(&chunk.data[..])
                    .position(|(a, b)| a != b)
            ));
        }

        for entry in package.entries() {
            let source = entry
                .name()
                .strip_suffix(".wem")
                .and_then(|id| id.parse::<u32>().ok());
            match source {
                Some(id) if entry.key() == PackageKey(u64::from(id)) => {}
                _ => failures.push(format!(
                    "{name}: entry {} has the key {:?}",
                    entry.name(),
                    entry.key()
                )),
            }
            let data = entry.data();
            if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
                not_riff_wave += 1;
            }
        }

        match MountedPackage::mount(Cursor::new(&chunk.data[..])) {
            Ok(mounted) if mounted.entries().len() == package.len() => {}
            Ok(_) => failures.push(format!("{name}: mount lists another entry count")),
            Err(e) => failures.push(format!("{name}: mount: {e}")),
        }
    });

    println!(
        "{packages} packages ({empty} empty), {entries} entries, {bytes} bytes, \
         {not_riff_wave} media not RIFF WAVE, {} failures",
        failures.len()
    );
    for failure in failures.iter().take(20) {
        println!("  {failure}");
    }
    assert!(packages > 0, "no packages found");
    assert!(failures.is_empty(), "{} packages failed", failures.len());
}
