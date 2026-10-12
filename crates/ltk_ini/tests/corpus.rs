//! Reads and writes every `.inibin`, `.troybin` and `.cfgbin` file in a directory. The test is
//! ignored unless it is run with `--ignored`, and it fails if `LTK_INI_DIR` is not set.
//!
//! ```text
//! LTK_INI_DIR="C:/path/to/extracted/DATA" \
//!     cargo test -p ltk_ini --test corpus --release -- --ignored --nocapture
//! ```
//!
//! The test fails if a file does not read to its last byte, if the written file is longer than
//! the file, or if the written file does not read back with the same values. The test also fails
//! if the bytes before the string offsets differ between the file and the written file.

use std::path::{Path, PathBuf};

use ltk_ini::{Inibin, ValueKind};

const INI_DIR: &str = "LTK_INI_DIR";

/// Appends the path of every binary ini file in `root` and its subdirectories to `found`.
fn ini_files(root: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            ini_files(&path, found);
        } else if path.extension().is_some_and(|e| {
            ["inibin", "troybin", "cfgbin"]
                .iter()
                .any(|ext| e.eq_ignore_ascii_case(ext))
        }) {
            found.push(path);
        }
    }
}

#[test]
#[ignore = "The test needs binary ini files. Set LTK_INI_DIR"]
fn inibin_round_trips_every_file_in_ini_dir() {
    let Ok(dir) = std::env::var(INI_DIR) else {
        panic!(
            "{INI_DIR} is not set. Set it to a directory that contains .inibin or .troybin files"
        );
    };
    let mut files = Vec::new();
    ini_files(Path::new(&dir), &mut files);
    assert!(!files.is_empty(), "{dir} contains no binary ini files");

    let (mut values, mut exact, mut shorter) = (0, 0, 0);
    let mut by_kind = [0usize; ValueKind::ALL.len()];
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let mut reader = bytes.as_slice();
        let inibin =
            Inibin::from_reader(&mut reader).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(reader.is_empty(), "{}: unread bytes", path.display());

        let mut written = Vec::new();
        inibin
            .to_writer(&mut written)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let back = Inibin::from_reader(&mut written.as_slice())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(back, inibin, "{}", path.display());

        let strings = inibin
            .values()
            .filter(|v| v.kind() == ValueKind::String)
            .count();
        let string_data_len = usize::from(u16::from_le_bytes([bytes[1], bytes[2]]));
        let offsets_start = bytes.len() - string_data_len - 2 * strings;
        // Bytes 1 and 2 are `string_data_len`. `to_writer` writes a smaller value if the file
        // stores a string more than once.
        assert_eq!(written[0], bytes[0], "{}", path.display());
        assert_eq!(
            written[3..offsets_start],
            bytes[3..offsets_start],
            "{}",
            path.display()
        );
        assert!(written.len() <= bytes.len(), "{}", path.display());

        values += inibin.len();
        for value in inibin.values() {
            by_kind[value.kind() as usize] += 1;
        }
        exact += usize::from(written == bytes);
        shorter += usize::from(written.len() < bytes.len());
    }

    println!(
        "{} files, {values} values: {exact} written files are equal to the file, {shorter} are shorter",
        files.len()
    );
    for (kind, count) in ValueKind::ALL.into_iter().zip(by_kind) {
        println!("  {kind:?}: {count}");
    }
}
