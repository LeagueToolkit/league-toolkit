use std::io::{Read, Write};

use byteorder::{ReadBytesExt as _, WriteBytesExt as _, LE};
use ltk_io_ext::{ReaderExt as _, WriterExt as _};

use crate::dir::{
    SharedScriptDir, CHARACTER_ROOT, CHARACTER_SUBDIRS, MAP_SCRIPT_EXTENSION, MAP_SCRIPT_SUBDIRS,
    SCRIPT_EXTENSIONS,
};
use crate::entry::ScriptEntry;
use crate::error::{LuaManifestError, Result};

/// Magic bytes at the start of every manifest: `"0MLR"` - i.e. `"RLM0"` stored
/// as a little-endian `u32`, which is how the game compares it.
///
/// Presumably short for `Riot Lua Manifest 0`; the binary carries no string
/// confirming that, and there is no version field anywhere in the file.
pub const MAGIC: [u8; 4] = *b"0MLR";

/// One character and the scripts scoped to it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CharacterScripts {
    /// The character (folder) name, e.g. `"aatrox"`.
    pub name: String,
    /// Bare script names - no directory, no extension.
    pub scripts: Vec<String>,
}

/// The `DATA/all_lua_files.manifest` file: the index of every Lua script the
/// game may load.
///
/// It has three sections, and the *only* one that yields exact paths on its own
/// is the shared one:
///
/// - **Characters** - per-character lists of bare script names. The game finds
///   these by probing [`CHARACTER_SUBDIRS`] under `DATA/Characters/<name>` in
///   order, so the manifest doesn't record which sub-directory each one is in.
/// - **Shared scripts** - a flat list of bare names, paired with…
/// - **Entries** - a sorted table of packed [`ScriptEntry`] values, each one a
///   truncated hash of a shared name plus the index of the directory it lives
///   in. Joining the two gives an exact *directory* - the extension is still a
///   guess, see [`shared_paths`](Self::shared_paths).
///
/// The two lists are only joined by hash, and every entry in a shipped file
/// matches some shared name - but a name can exist in *both* scopes (28 do in a
/// 16.x file, e.g. `CharScriptEvelynn`), so a lookup on a character-scoped name
/// can hit an unrelated shared entry. [`entry`](Self::entry) and everything
/// built on it answer the shared-scope question only.
///
/// Two kinds of shared name don't resolve, and they are not the same thing:
///
/// - **No entry at all** - map-scoped scripts, living under
///   `LEVELS/Map<N>/Scripts/…`. This is why the shared list is longer than the
///   entry table. See [`map_scoped_shared`](Self::map_scoped_shared).
/// - **An entry with the [`NO_SHARED_DIR`](crate::NO_SHARED_DIR) sentinel** -
///   a name the index knows but places nowhere; no map scripts are in this
///   group. See [`unplaced_shared`](Self::unplaced_shared).
///
/// # Reading
///
/// ```no_run
/// use ltk_lua::LuaManifest;
///
/// let mut file = std::fs::File::open("all_lua_files.manifest")?;
/// let manifest = LuaManifest::from_reader(&mut file)?;
///
/// for path in manifest.shared_paths() {
///     println!("{path}"); // DATA/Shared/Spells/Foo.luabin64
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LuaManifest {
    characters: Vec<CharacterScripts>,
    shared: Vec<String>,
    entries: Vec<ScriptEntry>,
}

impl LuaManifest {
    /// Creates an empty manifest.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a manifest from its three sections.
    ///
    /// The result is [`sort`](Self::sort)ed - the game binary-searches both the
    /// character list and the entry table.
    ///
    /// Nothing checks that `entries` and `shared` line up; that join is by hash
    /// and is the caller's to maintain (see [`ScriptEntry::new`]).
    #[must_use]
    pub fn from_parts(
        characters: Vec<CharacterScripts>,
        shared: Vec<String>,
        entries: Vec<ScriptEntry>,
    ) -> Self {
        let mut this = Self {
            characters,
            shared,
            entries,
        };
        this.sort();
        this
    }

    /// The per-character script lists.
    #[must_use]
    pub fn characters(&self) -> &[CharacterScripts] {
        &self.characters
    }

    /// The bare names of every shared (non character-scoped) script.
    #[must_use]
    pub fn shared_scripts(&self) -> &[String] {
        &self.shared
    }

    /// The packed hash table, sorted ascending.
    #[must_use]
    pub fn entries(&self) -> &[ScriptEntry] {
        &self.entries
    }

    /// Takes the three sections back out, for editing and re-assembling with
    /// [`from_parts`](Self::from_parts).
    #[must_use]
    pub fn into_parts(self) -> (Vec<CharacterScripts>, Vec<String>, Vec<ScriptEntry>) {
        (self.characters, self.shared, self.entries)
    }

    /// Sorts every section into the order shipped files use.
    ///
    /// All three are sorted by raw byte order (so `CHERRY` precedes
    /// `CHERRY_BlitzQ`, and uppercase sorts ahead of lowercase). The character
    /// list and the entry table *must* be sorted - the game binary-searches
    /// them. The shared list is observed sorted but is only iterated, and the
    /// per-character script lists are **not** sorted in shipped files, so
    /// neither is touched beyond the top level.
    pub fn sort(&mut self) {
        self.characters.sort_by(|a, b| a.name.cmp(&b.name));
        self.shared.sort();
        self.entries.sort_unstable();
    }

    /// Looks up the entry for a *shared* script name.
    ///
    /// Matching is by truncated hash, so this answers "is there a shared entry
    /// whose name hashes like this", not "is `name` in the shared list" - pass a
    /// character-scoped name that also exists as a shared one and you get the
    /// shared entry back.
    ///
    /// Requires [`entries`](Self::entries) to be sorted, which every constructor
    /// in this crate guarantees.
    #[must_use]
    pub fn entry(&self, name: &str) -> Option<ScriptEntry> {
        let key = ScriptEntry::key_of(name);
        self.entries
            .binary_search_by_key(&key, |e| e.key())
            .ok()
            .map(|i| self.entries[i])
    }

    /// The directory a shared script lives in, or [`None`] if it has no entry
    /// or its entry carries the sentinel index.
    #[must_use]
    pub fn dir_of(&self, name: &str) -> Option<SharedScriptDir> {
        self.entry(name)?.dir()
    }

    /// The full path of a shared script, e.g.
    /// `resolve_shared("Buff", "luabin64")` → `DATA/Spells/Modules/Buff.luabin64`.
    ///
    /// [`None`] for a name that isn't in the table or is map-scoped.
    #[must_use]
    pub fn resolve_shared(&self, name: &str, extension: &str) -> Option<String> {
        Some(format!("{}{name}.{extension}", self.dir_of(name)?))
    }

    /// Every shared script whose directory is known, crossed with both
    /// [`SCRIPT_EXTENSIONS`].
    ///
    /// The **directory is exact**; the **extension is not**. `.luabin64` always
    /// exists, but only some scripts have a `.preload` sidecar (roughly 60% in a
    /// 16.x build), so about a fifth of what this yields is not a real file.
    /// Filter against a WAD's path hashes, or use
    /// [`resolve_shared`](Self::resolve_shared) with a single extension.
    ///
    /// Names with no entry, or whose entry carries the sentinel, are skipped -
    /// see [`map_scoped_shared`](Self::map_scoped_shared) and
    /// [`unplaced_shared`](Self::unplaced_shared).
    pub fn shared_paths(&self) -> impl Iterator<Item = String> + '_ {
        self.shared.iter().flat_map(move |name| {
            let dir = self.dir_of(name);
            SCRIPT_EXTENSIONS
                .iter()
                .filter_map(move |ext| Some(format!("{}{name}.{ext}", dir?)))
        })
    }

    /// Shared script names with **no entry at all** — the reason the shared list
    /// is longer than the entry table.
    ///
    /// The format says nothing about what these are; what it says is that they
    /// have no directory. In shipped builds they are map-scoped - they live
    /// under `LEVELS/Map<N>/Scripts/` or `LEVELS/Map<N>/Scripts/Mutators/`, and
    /// neither the map id nor the `Mutators/` hop is recorded anywhere in the
    /// file - with a couple of names that ship nowhere at all. Cross them with
    /// the map ids you care about via [`map_paths`](Self::map_paths) and verify.
    pub fn map_scoped_shared(&self) -> impl Iterator<Item = &str> {
        self.shared
            .iter()
            .filter(|name| self.entry(name).is_none())
            .map(String::as_str)
    }

    /// Shared script names whose entry carries the
    /// [`NO_SHARED_DIR`](crate::NO_SHARED_DIR) sentinel instead of a directory
    /// index.
    ///
    /// The manifest knows the name but records no home for it, and the game
    /// treats such an entry exactly like a missing one: the lookup is rejected
    /// and resolution fails. In a 16.x build all of them are practice-tool
    /// `Cheat*` scripts with no file in any WAD, which suggests the sentinel
    /// means "the generator found nowhere to put this" - but that reading is an
    /// observation, not something the format states.
    pub fn unplaced_shared(&self) -> impl Iterator<Item = &str> {
        self.shared
            .iter()
            .filter(|name| self.entry(name).is_some_and(|e| e.dir().is_none()))
            .map(String::as_str)
    }

    /// Every *candidate* path for the [`map_scoped_shared`](Self::map_scoped_shared)
    /// names on map `map_id`, across both [`MAP_SCRIPT_SUBDIRS`].
    ///
    /// Only [`MAP_SCRIPT_EXTENSION`] is emitted - map scripts ship without a
    /// `.preload` sibling.
    ///
    /// Candidates, not paths: a given script lives on one map (or a handful),
    /// and nothing in the manifest says which. Verify against a WAD.
    pub fn map_paths(&self, map_id: u32) -> impl Iterator<Item = String> + '_ {
        self.map_scoped_shared().flat_map(move |name| {
            MAP_SCRIPT_SUBDIRS.iter().map(move |subdir| {
                format!("LEVELS/Map{map_id}{subdir}{name}.{MAP_SCRIPT_EXTENSION}")
            })
        })
    }

    /// Every *candidate* character-scoped path: each script crossed with all
    /// four [`CHARACTER_SUBDIRS`] and both [`SCRIPT_EXTENSIONS`].
    ///
    /// The game probes the sub-directories in order and takes the first that
    /// exists, so at most one per script is the one actually loaded. Filter the
    /// output against a WAD's path hashes to find out which; note a handful of
    /// scripts do ship in two of them, and `.preload` sidecars are optional.
    ///
    /// A character's script list may repeat a name, in which case so does this.
    pub fn character_paths(&self) -> impl Iterator<Item = String> + '_ {
        self.characters.iter().flat_map(|character| {
            character.scripts.iter().flat_map(move |script| {
                CHARACTER_SUBDIRS.iter().flat_map(move |subdir| {
                    SCRIPT_EXTENSIONS.iter().map(move |ext| {
                        format!(
                            "{CHARACTER_ROOT}{}{subdir}{script}.{ext}",
                            character.name.as_str()
                        )
                    })
                })
            })
        })
    }

    /// [`shared_paths`](Self::shared_paths) followed by
    /// [`character_paths`](Self::character_paths) - i.e. everything the manifest
    /// can place without being told a map id. Map-scoped names are *not*
    /// included; get those from [`map_paths`](Self::map_paths).
    ///
    /// Read the two methods' caveats: neither stream is a list of files that
    /// certainly exist.
    pub fn paths(&self) -> impl Iterator<Item = String> + '_ {
        self.shared_paths().chain(self.character_paths())
    }

    /// Reads a manifest.
    ///
    /// Sections are kept in the order they appear on disk, so a shipped file
    /// re-writes byte-identically. The one exception is the entry table: the
    /// game binary-searches it and so does [`entry`](Self::entry), so if a file
    /// somehow carries it unsorted it is sorted here rather than left to
    /// mis-resolve.
    pub fn from_reader(reader: &mut impl Read) -> Result<Self> {
        let mut magic = [0_u8; 4];
        reader.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(LuaManifestError::InvalidMagic {
                expected: MAGIC,
                actual: magic,
            });
        }

        let characters = read_vec(reader, |reader| {
            Ok(CharacterScripts {
                name: reader.read_sized_string_u32::<LE>()?,
                scripts: read_vec(reader, |reader| Ok(reader.read_sized_string_u32::<LE>()?))?,
            })
        })?;
        let shared = read_vec(reader, |reader| Ok(reader.read_sized_string_u32::<LE>()?))?;
        let mut entries = read_vec(reader, |reader| Ok(ScriptEntry(reader.read_u64::<LE>()?)))?;
        if !entries.windows(2).all(|w| w[0] <= w[1]) {
            entries.sort_unstable();
        }

        Ok(Self {
            characters,
            shared,
            entries,
        })
    }

    /// Writes the manifest.
    ///
    /// Sections are written in their current order - call
    /// [`sort`](Self::sort) first if you've edited them.
    pub fn to_writer(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_all(&MAGIC)?;

        write_len(writer, self.characters.len())?;
        for character in &self.characters {
            writer.write_sized_string_u32::<LE>(&character.name)?;
            write_len(writer, character.scripts.len())?;
            for script in &character.scripts {
                writer.write_sized_string_u32::<LE>(script)?;
            }
        }

        write_len(writer, self.shared.len())?;
        for name in &self.shared {
            writer.write_sized_string_u32::<LE>(name)?;
        }

        write_len(writer, self.entries.len())?;
        for entry in &self.entries {
            writer.write_u64::<LE>(entry.0)?;
        }

        Ok(())
    }
}

/// Reads a `u32`-counted list.
fn read_vec<R: Read, T>(
    reader: &mut R,
    mut read: impl FnMut(&mut R) -> Result<T>,
) -> Result<Vec<T>> {
    let count = reader.read_u32::<LE>()? as usize;
    // The count is attacker-controlled, so grow as we go instead of reserving it.
    let mut items = Vec::new();
    for _ in 0..count {
        items.push(read(reader)?);
    }
    Ok(items)
}

fn write_len(writer: &mut impl Write, len: usize) -> Result<()> {
    writer.write_u32::<LE>(u32::try_from(len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "list is longer than u32::MAX",
        )
    })?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> LuaManifest {
        LuaManifest::from_parts(
            vec![
                CharacterScripts {
                    name: "aatrox".into(),
                    scripts: vec!["AatroxE".into(), "CharScriptAatrox".into()],
                },
                CharacterScripts {
                    name: "ahri".into(),
                    scripts: vec!["AhriQ".into()],
                },
            ],
            vec![
                "Buff".into(),
                "1043".into(),
                "ButchersBridge".into(), // map-scoped: no entry
                "CheatAttackMe".into(),  // has an entry, but the sentinel index
            ],
            vec![
                ScriptEntry::new("Buff", Some(SharedScriptDir::SpellModules)),
                ScriptEntry::new("1043", Some(SharedScriptDir::Items)),
                ScriptEntry::new("CheatAttackMe", None),
            ],
        )
    }

    #[test]
    fn round_trips() {
        let manifest = manifest();
        let mut buf = Vec::new();
        manifest.to_writer(&mut buf).unwrap();

        assert_eq!(&buf[..4], b"0MLR");
        assert_eq!(
            LuaManifest::from_reader(&mut buf.as_slice()).unwrap(),
            manifest
        );
    }

    #[test]
    fn into_parts_round_trips() {
        let manifest = manifest();
        let (characters, shared, entries) = manifest.clone().into_parts();
        assert_eq!(
            LuaManifest::from_parts(characters, shared, entries),
            manifest
        );
    }

    #[test]
    fn sorts_an_unsorted_entry_table_on_read() {
        let mut manifest = manifest();
        manifest.entries.reverse(); // `entry` binary-searches; this would break it
        let mut buf = Vec::new();
        manifest.to_writer(&mut buf).unwrap();

        let read = LuaManifest::from_reader(&mut buf.as_slice()).unwrap();
        assert!(read.entries().windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(read.dir_of("1043"), Some(SharedScriptDir::Items));
    }

    #[test]
    fn rejects_bad_magic() {
        let err = LuaManifest::from_reader(&mut b"RLM1\0\0\0\0".as_slice()).unwrap_err();
        assert!(matches!(err, LuaManifestError::InvalidMagic { .. }));
    }

    #[test]
    fn resolves_shared_scripts() {
        let manifest = manifest();

        assert_eq!(
            manifest.resolve_shared("Buff", "luabin64").as_deref(),
            Some("DATA/Spells/Modules/Buff.luabin64")
        );
        assert_eq!(manifest.dir_of("1043"), Some(SharedScriptDir::Items));
        assert_eq!(manifest.dir_of("NotInTheTable"), None);

        // no entry at all -> map-scoped
        assert!(manifest.entry("ButchersBridge").is_none());
        assert_eq!(
            manifest.map_scoped_shared().collect::<Vec<_>>(),
            ["ButchersBridge"]
        );

        // an entry, but the sentinel index -> known name, no home
        assert!(manifest.entry("CheatAttackMe").is_some());
        assert_eq!(manifest.dir_of("CheatAttackMe"), None);
        assert_eq!(
            manifest.unplaced_shared().collect::<Vec<_>>(),
            ["CheatAttackMe"]
        );

        // emitted in shared-list order, which `sort` puts in byte order
        let paths = manifest.shared_paths().collect::<Vec<_>>();
        assert_eq!(
            paths,
            [
                "DATA/Items/1043.luabin64",
                "DATA/Items/1043.preload",
                "DATA/Spells/Modules/Buff.luabin64",
                "DATA/Spells/Modules/Buff.preload",
            ]
        );
    }

    #[test]
    fn enumerates_map_candidates() {
        assert_eq!(
            manifest().map_paths(12).collect::<Vec<_>>(),
            [
                "LEVELS/Map12/Scripts/ButchersBridge.luabin64",
                "LEVELS/Map12/Scripts/Mutators/ButchersBridge.luabin64",
            ]
        );
    }

    #[test]
    fn enumerates_character_candidates() {
        let paths = manifest().character_paths().collect::<Vec<_>>();

        assert_eq!(
            paths.len(),
            3 * CHARACTER_SUBDIRS.len() * SCRIPT_EXTENSIONS.len()
        );
        assert_eq!(paths[0], "DATA/Characters/aatrox/Spells/AatroxE.luabin64");
        assert_eq!(paths[1], "DATA/Characters/aatrox/Spells/AatroxE.preload");
        assert_eq!(paths[2], "DATA/Characters/aatrox/AatroxE.luabin64");
        assert_eq!(paths[4], "DATA/Characters/aatrox/Scripts/AatroxE.luabin64");
        assert_eq!(
            paths[6],
            "DATA/Characters/aatrox/NPCScripts/AatroxE.luabin64"
        );
        assert!(paths.contains(&"DATA/Characters/ahri/Spells/AhriQ.luabin64".to_string()));
    }
}
