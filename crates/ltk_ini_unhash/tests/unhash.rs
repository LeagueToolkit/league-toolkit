//! Tests `unhash` against shipped files and against files built key by key.

use std::path::Path;

use ltk_ini::{IniHash, IniTextOptions, Inibin, Value};
use ltk_ini_unhash::{
    to_ini, unhash, unhash_any, unhash_with_history, Class, History, Names, Options, Ty,
};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

fn read_fixture(name: &str) -> Inibin {
    let path = Path::new(FIXTURES).join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Inibin::from_reader(&mut bytes.as_slice()).unwrap()
}

/// Returns the single name of `section*name`, as `(section, name, ty, stale)`.
fn name_of<'n>(
    names: &'n Names,
    section: &str,
    name: &str,
) -> Option<(&'n str, &'n str, Option<Ty>, bool)> {
    let found = names.get(&IniHash::new(section, name))?;
    assert_eq!(found.len(), 1, "{section}*{name}: {found:?}");
    let n = &found[0];
    Some((n.section.as_str(), n.name.as_str(), n.ty, n.stale))
}

/// Returns the key of each property line of ini text, in order.
fn keys_of_ini(text: &str) -> Vec<IniHash> {
    let mut section = "";
    let mut keys = Vec::new();
    for line in text.lines() {
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|line| line.strip_suffix(']'))
        {
            section = name;
        } else if let Some((name, _)) = line.split_once('=') {
            keys.push(IniHash::new(section, name));
        }
    }
    keys
}

/// Names a shipped file. Asserts that `unhash_any` picks `class`, that the ini text names each key
/// of the file once, and that the ini text reads back to the keys and the text of each value.
/// Snapshots the ini text.
fn assert_names_shipped(fixture: &str, class: Class) {
    let inibin = read_fixture(fixture);

    let (picked, names) = unhash_any(&inibin, Options::default());
    let ini = to_ini(&inibin, &names);
    let read_back = Inibin::from_ini(&ini, IniTextOptions::default());

    assert_eq!(picked, class, "{fixture}");
    let mut keys = keys_of_ini(&ini);
    keys.sort();
    assert_eq!(
        keys,
        inibin.keys().copied().collect::<Vec<_>>(),
        "{fixture}"
    );
    let text = |inibin: &Inibin| -> Vec<(IniHash, String)> {
        inibin
            .iter()
            .map(|(key, value)| (*key, value.to_string()))
            .collect()
    };
    assert_eq!(text(&read_back), text(&inibin), "{fixture}");
    insta::assert_snapshot!(fixture, ini);
}

#[test]
fn unhash_any_names_shipped_unit_as_character() {
    assert_names_shipped("shen.inibin", Class::Character);
}

#[test]
fn unhash_any_names_shipped_skin_as_character() {
    assert_names_shipped("kalista_skin_base.inibin", Class::Character);
}

#[test]
fn unhash_any_names_shipped_spell_as_spell() {
    assert_names_shipped("dravenfury.inibin", Class::Spell);
}

#[test]
fn unhash_any_names_shipped_bot_file_as_globals() {
    assert_names_shipped("aatroxbotsummonerspellsclassic.inibin", Class::Globals);
}

#[test]
fn unhash_any_names_shipped_ui_file_as_ui() {
    assert_names_shipped("parstates.inibin", Class::Ui);
}

#[test]
fn unhash_any_names_shipped_particle_settings_as_particles() {
    assert_names_shipped("particlesdefault.inibin", Class::Particles);
}

#[test]
fn unhash_any_names_shipped_troybin_as_troybin() {
    assert_names_shipped(
        "global_item_travelsizeelixirofiron_buf.troybin",
        Class::Troybin,
    );
}

#[test]
fn unhash_any_names_shipped_terrain_as_level() {
    assert_names_shipped("map16_terrain.inibin", Class::Level);
}

#[test]
fn unhash_names_map_objects_from_object_list() {
    let mut inibin = Inibin::new();
    let mut header = inibin.section_mut("Header");
    header.insert("ObjectCount", 3);
    header.insert("Object1", "Turret_T1_C_01");
    header.insert("Object3", "LevelSize");
    let mut turret = inibin.section_mut("Turret_T1_C_01");
    turret.insert("Type", "Turret");
    turret.insert("Position", glam::Vec3::ZERO);
    turret.insert("LevelSize", glam::Vec3::ONE);
    let mut size = inibin.section_mut("LevelSize");
    size.insert("Type", "LevelSize");
    size.insert("LevelSize", glam::Vec3::ONE);

    let names = unhash(&inibin, Class::Scene, Options::default());

    assert_eq!(
        name_of(&names, "Turret_T1_C_01", "Position"),
        Some(("Turret_T1_C_01", "Position", Some(Ty::Vec3), false))
    );
    assert_eq!(
        name_of(&names, "LevelSize", "LevelSize"),
        Some(("LevelSize", "LevelSize", Some(Ty::Vec3), false))
    );
    assert!(name_of(&names, "Turret_T1_C_01", "LevelSize").is_none());
}

#[test]
fn unhash_ignores_ascii_case_of_section_names_from_values() {
    let mut inibin = Inibin::new();
    inibin.section_mut("System").insert("GroupPart1", "GLOW");
    inibin.section_mut("glow").insert("p-life", 1.5);

    let names = unhash(&inibin, Class::Troybin, Options::default());

    assert_eq!(
        name_of(&names, "glow", "p-life"),
        Some(("GLOW", "p-life", Some(Ty::Float), false))
    );
}

#[test]
fn unhash_names_emitter_from_integer_value() {
    let mut inibin = Inibin::new();
    inibin.section_mut("System").insert("GroupPart1", 123451);
    inibin.section_mut("123451").insert("p-life", 1.5);

    let names = unhash(&inibin, Class::Troybin, Options::default());

    assert_eq!(
        name_of(&names, "123451", "p-life"),
        Some(("123451", "p-life", Some(Ty::Float), false))
    );
}

#[test]
fn unhash_names_idle_particles_past_count_as_stale() {
    let mut inibin = Inibin::new();
    let mut idle = inibin.section_mut("IdleParticles");
    idle.insert("NumberOfParticles", 1);
    idle.insert("Particle1", "glow.troy");
    idle.insert("Particle3", "spark.troy");

    let names = unhash(&inibin, Class::Character, Options::default());

    assert_eq!(
        name_of(&names, "IdleParticles", "Particle1"),
        Some(("IdleParticles", "Particle1", Some(Ty::String), false))
    );
    assert_eq!(
        name_of(&names, "IdleParticles", "Particle3"),
        Some(("IdleParticles", "Particle3", Some(Ty::String), true))
    );
}

/// A troybin with a part that the client reads, a part after a gap, and a part with a comment
/// prefix.
fn troybin_with_stale_parts() -> Inibin {
    let mut inibin = Inibin::new();
    let mut system = inibin.section_mut("System");
    for i in 1..=8 {
        system.insert(&format!("GroupPart{i}"), format!("part{i}"));
    }
    system.insert("GroupPart10", "late");
    system.insert("'GroupPart9", "off");
    for emitter in ["part1", "late", "off"] {
        inibin.section_mut(emitter).insert("p-life", 1.5);
    }
    inibin
}

#[test]
fn unhash_names_stale_and_commented_keys_by_default() {
    let names = unhash(
        &troybin_with_stale_parts(),
        Class::Troybin,
        Options::default(),
    );

    assert_eq!(
        name_of(&names, "part1", "p-life"),
        Some(("part1", "p-life", Some(Ty::Float), false))
    );
    assert_eq!(
        name_of(&names, "System", "GroupPart10"),
        Some(("System", "GroupPart10", Some(Ty::String), true))
    );
    assert_eq!(
        name_of(&names, "late", "p-life"),
        Some(("late", "p-life", Some(Ty::Float), true))
    );
    let commented = &names[&IniHash::new("System", "'GroupPart9")][0];
    assert_eq!(commented.name, "'GroupPart9");
    assert_eq!(commented.comment(), Some("'"));
    assert!(commented.stale);
    assert_eq!(
        name_of(&names, "off", "p-life"),
        Some(("off", "p-life", Some(Ty::Float), true))
    );
}

#[test]
fn unhash_with_client_options_names_only_client_reads() {
    let names = unhash(
        &troybin_with_stale_parts(),
        Class::Troybin,
        Options::client(),
    );

    assert!(name_of(&names, "part1", "p-life").is_some());
    assert!(name_of(&names, "System", "GroupPart10").is_none());
    assert!(name_of(&names, "late", "p-life").is_none());
    assert!(!names.contains_key(&IniHash::new("System", "'GroupPart9")));
    assert!(names.values().flatten().all(|name| !name.stale));
}

#[test]
fn unhash_with_options_excludes_commented_keys_only() {
    let options = Options {
        commented: false,
        ..Options::default()
    };

    let names = unhash(&troybin_with_stale_parts(), Class::Troybin, options);

    assert!(name_of(&names, "System", "GroupPart10").is_some());
    assert!(!names.contains_key(&IniHash::new("System", "'GroupPart9")));
}

#[test]
fn unhash_with_history_names_emitter_of_other_versions() {
    let mut inibin = Inibin::new();
    inibin.section_mut("System").insert("GroupPart1", "glow");
    inibin.section_mut("removed").insert("p-life", 2.0);
    let history = History::from([(
        IniHash::new("System", "GroupPart2"),
        vec!["removed".to_owned()],
    )]);

    let without = unhash(&inibin, Class::Troybin, Options::default());
    let with = unhash_with_history(&inibin, Class::Troybin, Options::default(), &history);

    assert!(name_of(&without, "removed", "p-life").is_none());
    assert_eq!(
        name_of(&with, "removed", "p-life"),
        Some(("removed", "p-life", Some(Ty::Float), true))
    );
}

#[test]
fn unhash_names_flag_as_int() {
    let mut inibin = Inibin::new();
    inibin
        .section_mut("SpellData")
        .insert("IsToggleSpell", Value::U1(true));

    let names = unhash(&inibin, Class::Spell, Options::default());

    assert_eq!(
        name_of(&names, "SpellData", "IsToggleSpell"),
        Some(("SpellData", "IsToggleSpell", Some(Ty::Int), false))
    );
}
