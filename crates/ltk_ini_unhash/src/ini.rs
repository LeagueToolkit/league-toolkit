//! The ini text of a named file.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use ltk_hash::{sdbm, IniHash};
use ltk_ini::{Inibin, Value};

use crate::Names;

/// The section of the keys without a name in [`to_ini`].
pub const UNKNOWN_SECTION: &str = "unknown";

/// The characters of the suffix of an unknown property name.
const ALPHABET: &[u8; 36] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// The length of the tail of a suffix. [`tails`] holds every tail of this length.
const TAIL: usize = 3;

/// The text of a section name, and the property names and values of the section.
type Section<'a> = (&'a str, Vec<(&'a str, &'a Value)>);

/// Returns `inibin` as ini text, with the names of `names`.
///
/// A key with a name is a line `name=value` in the section of its first name. A key without a
/// name is a line in the section `[unknown]`, after every other section. Its property name is
/// `0x`, the key in hex, `_`, and a suffix of lowercase letters and digits. The key of
/// `unknown*` and that name is the key. Sections and properties are in ascending order of name,
/// ignoring ASCII case, with a run of digits ordered by its number. A value is the text of its
/// [`Display`](std::fmt::Display).
///
/// Ini text holds no declared type, no stale flag and no other names of a key.
#[must_use]
pub fn to_ini(inibin: &Inibin, names: &Names) -> String {
    let mut sections: BTreeMap<String, Section<'_>> = BTreeMap::new();
    let mut unknown = Vec::new();
    for (key, value) in inibin {
        match names.get(key).and_then(|names| names.first()) {
            Some(name) => sections
                .entry(name.section.to_ascii_lowercase())
                .or_insert_with(|| (name.section.as_str(), Vec::new()))
                .1
                .push((name.name.as_str(), value)),
            None => unknown.push((*key, value)),
        }
    }
    let mut out = String::new();
    let mut sections: Vec<Section<'_>> = sections.into_values().collect();
    sections.sort_by(|a, b| natural_cmp(a.0, b.0));
    for (section, mut properties) in sections {
        properties.sort_by(|a, b| natural_cmp(a.0, b.0));
        write_section(&mut out, section);
        for (name, value) in properties {
            // Writing to a `String` never fails.
            let _ = writeln!(out, "{name}={value}");
        }
    }
    if !unknown.is_empty() {
        write_section(&mut out, UNKNOWN_SECTION);
        for (key, value) in unknown {
            let _ = writeln!(out, "{}={value}", unknown_name(key));
        }
    }
    out
}

/// Orders names ignoring ASCII case, with a run of digits ordered by its number: `Particle2`
/// before `Particle10`. Of two equal numbers, the shorter run comes first: `1` before `01`.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i].is_ascii_digit() && b[j].is_ascii_digit() {
            let run_a = a[i..].iter().take_while(|c| c.is_ascii_digit()).count();
            let run_b = b[j..].iter().take_while(|c| c.is_ascii_digit()).count();
            let (num_a, num_b) = (&a[i..i + run_a], &b[j..j + run_b]);
            let trim = |num: &[u8]| {
                let zeros = num.iter().take_while(|&&c| c == b'0').count();
                num.len() - zeros
            };
            let (len_a, len_b) = (trim(num_a), trim(num_b));
            let order = len_a
                .cmp(&len_b)
                .then_with(|| num_a[run_a - len_a..].cmp(&num_b[run_b - len_b..]))
                .then_with(|| run_a.cmp(&run_b));
            if order != Ordering::Equal {
                return order;
            }
            i += run_a;
            j += run_b;
        } else {
            let order = a[i].to_ascii_lowercase().cmp(&b[j].to_ascii_lowercase());
            if order != Ordering::Equal {
                return order;
            }
            i += 1;
            j += 1;
        }
    }
    (a.len() - i).cmp(&(b.len() - j))
}

fn write_section(out: &mut String, section: &str) {
    if !out.is_empty() {
        out.push('\n');
    }
    let _ = writeln!(out, "[{section}]");
}

/// Returns the property name of `key` in the section `unknown`: `0x`, the key in hex, `_`, and
/// the first suffix in order of length, then of [`ALPHABET`], whose name hashes to `key`.
///
/// The suffix is a head and a tail of [`TAIL`] characters. The search walks the heads and looks
/// up the hash that the tail needs in [`tails`]. SDBM is linear in its state: the tail needs
/// `key - state * MULTIPLIER^TAIL`.
fn unknown_name(key: IniHash) -> String {
    let prefix = format!("0x{:08x}_", key.0);
    let state = sdbm::hash_lower_from(sdbm::hash_lower(UNKNOWN_SECTION), "*");
    let state = sdbm::hash_lower_from(state, &prefix);
    let power = sdbm::MULTIPLIER.wrapping_pow(TAIL as u32);
    let tails = tails();
    let find = |head_state: u32| tails.find(key.0.wrapping_sub(head_state.wrapping_mul(power)));
    let name = |head: &[usize], tail: [u8; TAIL]| {
        let mut name = prefix.clone();
        name.extend(head.iter().map(|&digit| char::from(ALPHABET[digit])));
        name.extend(tail.iter().map(|&byte| char::from(byte)));
        name
    };
    if let Some(tail) = find(state) {
        return name(&[], tail);
    }
    // The heads with the parent `parent` and each last character, in order.
    let mut parent = Vec::new();
    loop {
        let parent_state = parent.iter().fold(state, |state, &digit| {
            sdbm::hash_lower_byte(state, ALPHABET[digit])
        });
        for (last, &byte) in ALPHABET.iter().enumerate() {
            if let Some(tail) = find(sdbm::hash_lower_byte(parent_state, byte)) {
                parent.push(last);
                return name(&parent, tail);
            }
        }
        next_head(&mut parent);
    }
}

/// Advances `head` to the next head in order of length, then of [`ALPHABET`].
fn next_head(head: &mut Vec<usize>) {
    for digit in head.iter_mut().rev() {
        *digit += 1;
        if *digit < ALPHABET.len() {
            return;
        }
        *digit = 0;
    }
    head.push(0);
}

/// Every tail of [`TAIL`] characters with its SDBM hash from the state 0.
struct Tails {
    /// The tails, sorted by hash, then by tail.
    entries: Vec<(u32, [u8; TAIL])>,
    /// The index of the first entry of each value of the high 16 bits of a hash, and the number
    /// of entries.
    starts: Vec<u32>,
}

impl Tails {
    /// Returns the first tail whose hash is `hash`.
    fn find(&self, hash: u32) -> Option<[u8; TAIL]> {
        let bucket = (hash >> 16) as usize;
        let range = self.starts[bucket] as usize..self.starts[bucket + 1] as usize;
        self.entries[range]
            .iter()
            .find(|&&(entry, _)| entry == hash)
            .map(|&(_, tail)| tail)
    }
}

fn tails() -> &'static Tails {
    static TAILS: OnceLock<Tails> = OnceLock::new();
    TAILS.get_or_init(|| {
        let mut entries = Vec::with_capacity(ALPHABET.len().pow(TAIL as u32));
        for &a in ALPHABET {
            for &b in ALPHABET {
                for &c in ALPHABET {
                    let tail = [a, b, c];
                    let hash = tail
                        .iter()
                        .fold(0, |state, &byte| sdbm::hash_lower_byte(state, byte));
                    entries.push((hash, tail));
                }
            }
        }
        entries.sort_unstable();
        let starts = (0..=1u32 << 16)
            .map(|bucket| {
                let first = u64::from(bucket) << 16;
                entries.partition_point(|&(hash, _)| u64::from(hash) < first) as u32
            })
            .collect();
        Tails { entries, starts }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_name_hashes_to_key() {
        for key in [0, 1, 0x158dd84c, 0xdeadbeef, u32::MAX] {
            let name = unknown_name(IniHash(key));
            assert!(name.starts_with(&format!("0x{key:08x}_")), "{name}");
            assert_eq!(IniHash::new(UNKNOWN_SECTION, &name), IniHash(key), "{name}");
        }
    }

    #[test]
    fn natural_cmp_orders_numbers_by_value() {
        let mut names = [
            "Particle10",
            "particle2",
            "Particle1",
            "Particle01",
            "Part",
            "PartX",
        ];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            [
                "Part",
                "Particle1",
                "Particle01",
                "particle2",
                "Particle10",
                "PartX"
            ]
        );
    }

    #[test]
    fn next_head_counts_in_order_of_length() {
        let mut head = Vec::new();
        next_head(&mut head);
        assert_eq!(head, [0]);
        head = vec![35];
        next_head(&mut head);
        assert_eq!(head, [0, 0]);
        head = vec![0, 35];
        next_head(&mut head);
        assert_eq!(head, [1, 0]);
    }
}
