use xxhash_rust::xxh3::xxh3_64;

use crate::dir::SharedScriptDir;

/// Number of low bits of a [`ScriptEntry`] taken up by the directory index.
pub const DIR_INDEX_BITS: u32 = 5;

/// Mask of the directory-index bits of a [`ScriptEntry`].
pub const DIR_INDEX_MASK: u64 = (1 << DIR_INDEX_BITS) - 1;

/// The directory index found on entries that aren't in any of the 16
/// [`SharedScriptDir`]s.
///
/// The index field is 5 bits wide, so it can hold 0..=31, but only 0..=15 are
/// table slots — the resolver rejects everything `>= 16`. An entry carrying this
/// value resolves to nothing at all, exactly as if it were absent.
///
/// 16 is the only out-of-range value seen in shipped files, and it is what
/// [`ScriptEntry::new`] writes; 17..=31 would behave identically, and
/// [`ScriptEntry::dir`] treats them the same way.
///
/// It is not a marker for map-scoped scripts: those carry no entry whatsoever.
pub const NO_SHARED_DIR: u8 = 16;

/// `XXH3-64` (default secret, seed 0) of the lowercased script name - the hash
/// the manifest's entries are keyed by.
///
/// This is *not* the WAD path hash; it hashes the bare script name (no
/// directory, no extension).
///
/// Lowercasing is ASCII-only, matching the game's `tolower`. Every name in a
/// shipped manifest is ASCII, so the distinction has never mattered in practice.
#[must_use]
pub fn hash_script_name(name: &str) -> u64 {
    xxh3_64(name.to_ascii_lowercase().as_bytes())
}

/// A packed shared-script table entry: a truncated name hash plus the index of
/// the directory the script lives in.
///
/// ```text
/// entry = (XXH3_64(lowercase(name)) << 5) | dir_index
///          bits 5..63                        bits 0..4
/// ```
///
/// The shift means the top 5 bits of the hash are discarded, so an entry can
/// only be matched against a *candidate name* - it can't be inverted.
#[repr(transparent)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScriptEntry(pub u64);

impl ScriptEntry {
    /// Builds the entry for `name`, placed in `dir` ([`None`] for the
    /// [`NO_SHARED_DIR`] sentinel).
    #[must_use]
    pub fn new(name: &str, dir: Option<SharedScriptDir>) -> Self {
        let index = dir.map_or(NO_SHARED_DIR, u8::from);
        Self(Self::key_of(name) | u64::from(index))
    }

    /// The lookup key for `name`: its hash shifted into place, with the
    /// directory-index bits zeroed.
    #[must_use]
    pub fn key_of(name: &str) -> u64 {
        hash_script_name(name) << DIR_INDEX_BITS
    }

    /// This entry's lookup key - i.e. the entry with its directory index masked
    /// off. Two entries with the same key name the same script.
    #[must_use]
    pub const fn key(self) -> u64 {
        self.0 & !DIR_INDEX_MASK
    }

    /// The raw directory index, including the [`NO_SHARED_DIR`] sentinel.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn dir_index(self) -> u8 {
        (self.0 & DIR_INDEX_MASK) as u8
    }

    /// The directory this script lives in, or [`None`] if the entry carries the
    /// [`NO_SHARED_DIR`] sentinel.
    #[must_use]
    pub fn dir(self) -> Option<SharedScriptDir> {
        SharedScriptDir::try_from(self.dir_index()).ok()
    }

    /// Whether this entry is the one for `name`.
    #[must_use]
    pub fn matches(self, name: &str) -> bool {
        self.key() == Self::key_of(name)
    }
}

impl From<u64> for ScriptEntry {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl From<ScriptEntry> for u64 {
    fn from(value: ScriptEntry) -> Self {
        value.0
    }
}

impl std::fmt::LowerHex for ScriptEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Golden values lifted from a real 16.x `DATA/all_lua_files.manifest`.
    #[test]
    fn packs_like_the_game() {
        assert_eq!(hash_script_name("Buff"), 0x4f8b_a9da_c274_1602);

        let entry = ScriptEntry(0xf175_3b58_4e82_c041);
        assert!(entry.matches("Buff"));
        assert!(entry.matches("bUfF")); // name hashing is case-insensitive
        assert_eq!(entry.dir(), Some(SharedScriptDir::SpellModules));
        assert_eq!(
            entry,
            ScriptEntry::new("Buff", Some(SharedScriptDir::SpellModules))
        );

        let entry = ScriptEntry(0x8160_51ba_183d_c3eb);
        assert!(entry.matches("1043"));
        assert_eq!(entry.dir(), Some(SharedScriptDir::Items));
    }

    #[test]
    fn sentinel_has_no_dir() {
        let entry = ScriptEntry::new("ARAMCompanionMutator", None);
        assert_eq!(entry.dir_index(), NO_SHARED_DIR);
        assert_eq!(entry.dir(), None);
        assert!(entry.matches("aramcompanionmutator"));
    }
}
