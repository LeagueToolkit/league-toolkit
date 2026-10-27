//! Section and property names for the keys of `.inibin`, `.troybin` and `.cfgbin` files.
//!
//! An inibin file stores the hash of each section name and property name, and no names. A
//! [`Class`] has a recipe: a description of the sections and properties that the client loaders
//! of a family of files read. A read of a key that the file has names the key. A recipe names
//! some sections from values of the file: a `.troybin` file names its emitter sections in
//! `[System] GroupPart1`, `GroupPart2`, and so on.
//!
//! ```no_run
//! use std::fs::File;
//! use ltk_ini::Inibin;
//! use ltk_ini_unhash::{unhash, Class, Options};
//!
//! let troybin = Inibin::from_reader(&mut File::open("flash.troybin")?)?;
//! for (key, names) in unhash(&troybin, Class::Troybin, Options::default()) {
//!     println!("{key:08x} = [{}] {}", names[0].section, names[0].name);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Stale and commented keys
//!
//! A file can hold keys that the client never reads: typos, parts after a gap in a numbered list,
//! keys of a removed emitter. A recipe reads them on a stale path, and their [`Name::stale`] is
//! `true`. The ini compiler of old builds stores a line that starts with a comment prefix as a
//! property: `'GroupPart3` ([`COMMENT_PREFIXES`]). The names of such keys have the prefix.
//!
//! [`Options::default`] names every key. [`Options::client`] names only the keys that the client
//! reads.
//!
//! # Ini text
//!
//! [`to_ini`] writes a named file as ini text. A key without a name gets a property name in the
//! section `[unknown]` that hashes to the key.

mod ini;
mod name;
mod reader;
mod recipes;

use std::collections::{BTreeMap, HashMap};

use ltk_hash::IniHash;
use ltk_ini::Inibin;

pub use ini::{to_ini, UNKNOWN_SECTION};
pub use name::{Name, Options, Ty, COMMENT_PREFIXES};

use reader::Reader;

/// A family of inibin files that one recipe names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Class {
    /// Characters, skins, materials and the shop files of a character.
    Character,
    /// Spells, buffs, talents, items and item groups.
    Spell,
    /// `DATA/Globals` files, bot loadouts and item categories.
    Globals,
    /// Loading screen, HUD, bars and game bar animations.
    Ui,
    /// Particle settings and `.prtbin` particles.
    Particles,
    /// `.troybin` particle systems.
    Troybin,
    /// The files of a level: audio, graphics, terrain, timers, navigation.
    Level,
    /// The scene and the map objects of a level.
    Scene,
}

impl Class {
    /// Every class.
    pub const ALL: &'static [Class] = &[
        Class::Character,
        Class::Spell,
        Class::Globals,
        Class::Ui,
        Class::Particles,
        Class::Troybin,
        Class::Level,
        Class::Scene,
    ];

    fn read(self, r: &mut Reader) {
        match self {
            Class::Character => recipes::character::read(r),
            Class::Spell => recipes::spell::read(r),
            Class::Globals => recipes::globals::read(r),
            Class::Ui => recipes::ui::read(r),
            Class::Particles => recipes::particles::read(r),
            Class::Troybin => recipes::troybin::read(r),
            Class::Level => recipes::level::read(r),
            Class::Scene => recipes::scene::read(r),
        }
    }
}

/// The string values that other versions of a file hold, by key.
///
/// A `.troybin` file names its emitters in `[System] GroupPartN`. A file can keep the section of an
/// emitter whose `GroupPart` line another version of the file lists. A history names that section.
pub type History = HashMap<IniHash, Vec<String>>;

/// The names of the keys of a file, in ascending key order.
///
/// A key has more than one name if names of equal rank collide. A name that the client reads
/// ranks before a stale name. A name without a comment prefix ranks before a name with a comment
/// prefix.
pub type Names = BTreeMap<IniHash, Vec<Name>>;

/// Names the keys of `inibin` with the recipe of `class`.
#[must_use]
pub fn unhash(inibin: &Inibin, class: Class, options: Options) -> Names {
    run(inibin, class, options, None)
}

/// Names the keys of `inibin` as [`unhash`] does. `history` holds the values of the other versions
/// of the file ([`History`]).
#[must_use]
pub fn unhash_with_history(
    inibin: &Inibin,
    class: Class,
    options: Options,
    history: &History,
) -> Names {
    run(inibin, class, options, Some(history))
}

/// Names the keys of `inibin` with the recipe of each class. Returns the class that names the
/// most keys, and its names. Returns the first such class of [`Class::ALL`] on a tie.
#[must_use]
pub fn unhash_any(inibin: &Inibin, options: Options) -> (Class, Names) {
    let mut best = (Class::ALL[0], Names::new());
    for &class in Class::ALL {
        let names = unhash(inibin, class, options);
        if names.len() > best.1.len() {
            best = (class, names);
        }
    }
    best
}

fn run(inibin: &Inibin, class: Class, options: Options, history: Option<&History>) -> Names {
    let mut reader = Reader::new(inibin, history);
    class.read(&mut reader);
    reader.finish(options)
}
