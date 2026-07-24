use num_enum::{IntoPrimitive, TryFromPrimitive};

/// One of the 16 hardcoded directories a *shared* script can live in.
///
/// The manifest never stores a shared script's directory as text - it stores an
/// index into this table, packed into the low bits of the script's hash entry
/// (see [`ScriptEntry`](crate::ScriptEntry)).  The table is baked into the game
/// binary, so the order of these variants is part of the file format and must not change.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TryFromPrimitive, IntoPrimitive,
)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum SharedScriptDir {
    /// `DATA/Spells/`
    Spells = 0,
    /// `DATA/Spells/Modules/`
    SpellModules = 1,
    /// `DATA/Scripts/`
    Scripts = 2,
    /// `DATA/Shared/Scripts/`
    SharedScripts = 3,
    /// `DATA/Shared/Scripts/AIComponents/`
    SharedAiComponents = 4,
    /// `DATA/Shared/Spells/`
    SharedSpells = 5,
    /// `DATA/Shared/NPCScripts/`
    SharedNpcScripts = 6,
    /// `DATA/Shared/TFT/Common/`
    TftCommon = 7,
    /// `DATA/Shared/TFT/Items/`
    TftItems = 8,
    /// `DATA/Shared/TFT/Traits/`
    TftTraits = 9,
    /// `DATA/Shared/Spells/PracticeTool/`
    PracticeToolSpells = 10,
    /// `DATA/Items/`
    Items = 11,
    /// `DATA/Items/Spells/`
    ItemSpells = 12,
    /// `DATA/Items/Spells/Modules/`
    ItemSpellModules = 13,
    /// `DATA/BuildingBlocks/`
    BuildingBlocks = 14,
    /// `DATA/Shared/GameModes/`
    GameModes = 15,
}

impl SharedScriptDir {
    /// Every directory, in index order.
    pub const ALL: [Self; 16] = [
        Self::Spells,
        Self::SpellModules,
        Self::Scripts,
        Self::SharedScripts,
        Self::SharedAiComponents,
        Self::SharedSpells,
        Self::SharedNpcScripts,
        Self::TftCommon,
        Self::TftItems,
        Self::TftTraits,
        Self::PracticeToolSpells,
        Self::Items,
        Self::ItemSpells,
        Self::ItemSpellModules,
        Self::BuildingBlocks,
        Self::GameModes,
    ];

    /// The path prefix, with a trailing slash - e.g. `"DATA/Shared/Spells/"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Spells => "DATA/Spells/",
            Self::SpellModules => "DATA/Spells/Modules/",
            Self::Scripts => "DATA/Scripts/",
            Self::SharedScripts => "DATA/Shared/Scripts/",
            Self::SharedAiComponents => "DATA/Shared/Scripts/AIComponents/",
            Self::SharedSpells => "DATA/Shared/Spells/",
            Self::SharedNpcScripts => "DATA/Shared/NPCScripts/",
            Self::TftCommon => "DATA/Shared/TFT/Common/",
            Self::TftItems => "DATA/Shared/TFT/Items/",
            Self::TftTraits => "DATA/Shared/TFT/Traits/",
            Self::PracticeToolSpells => "DATA/Shared/Spells/PracticeTool/",
            Self::Items => "DATA/Items/",
            Self::ItemSpells => "DATA/Items/Spells/",
            Self::ItemSpellModules => "DATA/Items/Spells/Modules/",
            Self::BuildingBlocks => "DATA/BuildingBlocks/",
            Self::GameModes => "DATA/Shared/GameModes/",
        }
    }
}

impl std::fmt::Display for SharedScriptDir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The sub-directories tried, in order, under `DATA/Characters/<character>` when
/// resolving a character-scoped script.
///
/// The game probes these against the VFS and takes the first hit, so the
/// manifest gives no way to know which one a given script actually uses -
/// see [`LuaManifest::character_paths`](crate::LuaManifest::character_paths).
/// Almost every script resolves in exactly one of them, but a few do ship in
/// two, in which case only the first listed here is the one that loads.
///
/// Like [`SharedScriptDir`], this table is hardcoded - its contents and order
/// are part of the format.
pub const CHARACTER_SUBDIRS: [&str; 4] = ["/Spells/", "/", "/Scripts/", "/NPCScripts/"];

/// The root every character-scoped path is built under.
pub const CHARACTER_ROOT: &str = "DATA/Characters/";

/// The sub-directories a map-scoped script can sit in, under `LEVELS/Map<N>`.
///
/// Unlike the two tables above this one isn't a lookup the format performs -
/// map-scoped scripts carry no index at all, so both are candidates. They are
/// also reached by two different loaders: level scripts by the map context, and
/// mutators by a loader that builds the whole `Mutators/` path itself.
///
/// Observed in shipped data on maps 11, 12, 21, 22, 30, 33 and 35, and a given
/// script sits in one of the two - never both.
pub const MAP_SCRIPT_SUBDIRS: [&str; 2] = ["/Scripts/", "/Scripts/Mutators/"];

/// The only extension map-scoped scripts are shipped with.
///
/// Both loaders that reach them resolve with `luabin64`; no `LEVELS/…` path in
/// shipped data has a `.preload` sibling, unlike shared and character scripts.
pub const MAP_SCRIPT_EXTENSION: &str = "luabin64";

/// The extensions the game appends to a resolved script name: compiled bytecode
/// and its preload sidecar.
///
/// `luabin64` is always present for a script that ships at all; `preload` is
/// optional and absent for a large minority of them. Crossing a name with both
/// gives candidates, not files.
pub const SCRIPT_EXTENSIONS: [&str; 2] = ["luabin64", "preload"];
