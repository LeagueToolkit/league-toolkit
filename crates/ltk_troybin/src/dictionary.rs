//! Property name dictionaries for troybin hash resolution.
//!
//! Ported from Leischii's TroybinConverter dictionary and
//! Rey's IniHashDictionary.cs. These dictionaries enumerate all
//! known troybin property names so their ihashes can be computed
//! and matched against the hashes in the binary data.

use std::collections::HashMap;

use crate::hash::{build_hash_entries, section_field_hash};

// ── Expansion helpers ────────────────────────────────────────────────────────

fn generate_list(base: &[&str], start: usize, end: usize) -> Vec<String> {
    let mut result = Vec::new();
    for &item in base {
        if item.contains("%N%") {
            for k in start..end {
                result.push(item.replace("%N%", &k.to_string()));
            }
        } else {
            result.push(item.to_string());
        }
    }
    result
}

fn flex_names(args: &[&str]) -> Vec<String> {
    let mut result = Vec::new();
    for &a in args {
        result.push(a.to_string());
        result.push(format!("{}_flex", a));
        for j in 0..4 {
            result.push(format!("{}_flex{}", a, j));
        }
    }
    result
}

fn rand_names(mods: &[&str], args: &[String]) -> Vec<String> {
    let mut result: Vec<String> = args.to_vec();
    for arg in args {
        for j in 0..10 {
            result.push(format!("{}{}", arg, j));
        }
        for m in mods {
            result.push(format!("{}{}P", arg, m));
            for l in 0..10 {
                result.push(format!("{}{}P{}", arg, m, l));
            }
        }
    }
    result
}

fn color_names(mods: &[&str], args: &[String]) -> Vec<String> {
    let mut result: Vec<String> = args.to_vec();
    for arg in args {
        for j in 0..25 {
            result.push(format!("{}{}", arg, j));
        }
        for m in mods {
            result.push(format!("{}{}P", arg, m));
            for l in 0..25 {
                result.push(format!("{}{}P{}", arg, m, l));
            }
        }
    }
    result
}

fn rand_float(args: &[String]) -> Vec<String> {
    rand_names(&["X", ""], args)
}
fn rand_vec2(args: &[String]) -> Vec<String> {
    rand_names(&["X", "Y"], args)
}
fn rand_vec3(args: &[String]) -> Vec<String> {
    rand_names(&["X", "Y", "Z"], args)
}
fn rand_color(args: &[String]) -> Vec<String> {
    rand_names(&["R", "G", "B", "A"], args)
}
fn rand_color_amount(args: &[String]) -> Vec<String> {
    color_names(&["R", "G", "B", "A"], args)
}

fn flex_rand_float(args: &[&str]) -> Vec<String> {
    rand_float(&flex_names(args))
}
fn flex_rand_vec2(args: &[&str]) -> Vec<String> {
    rand_vec2(&flex_names(args))
}
fn flex_rand_vec3(args: &[&str]) -> Vec<String> {
    rand_vec3(&flex_names(args))
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

// ── Dictionary sections ──────────────────────────────────────────────────────

const MATERIAL_NAMES: &[&str] = &[
    "MaterialOverrideTransMap",
    "MaterialOverrideTransSource",
    "p-trans-sample",
    "MaterialOverride%N%BlendMode",
    "MaterialOverride%N%GlossTexture",
    "MaterialOverride%N%EmissiveTexture",
    "MaterialOverride%N%FixedAlphaScrolling",
    "MaterialOverride%N%Priority",
    "MaterialOverride%N%RenderingMode",
    "MaterialOverride%N%SubMesh",
    "MaterialOverride%N%Texture",
    "MaterialOverride%N%UVScroll",
];

const PART_FLUID_NAMES: &[&str] = &["fluid-params"];
const _PART_GROUP_NAMES: &[&str] = &["GroupPart%N%"];
const PART_FIELD_NAMES: &[&str] = &[
    "field-accel-%N%",
    "field-attract-%N%",
    "field-drag-%N%",
    "field-noise-%N%",
    "field-orbit-%N%",
];
const FIELD_NAMES_BASE: &[&str] = &["f-localspace", "f-axisfrac"];

fn get_system_names() -> Vec<String> {
    let base: &[&str] = &[
        "AudioFlexValueParameterName",
        "AudioParameterFlexID",
        "build-up-time",
        "group-vis",
        "group-scale-cap",
        "GroupPart%N%",
        "GroupPart%N%Type",
        "GroupPart%N%Importance",
        "Override-Offset%N%",
        "Override-Rotation%N%",
        "Override-Scale%N%",
        "KeepOrientationAfterSpellCast",
        "PersistThruDeath",
        "PersistThruRevive",
        "SelfIllumination",
        "SimulateEveryFrame",
        "SimulateOncePerFrame",
        "SimulateWhileOffScreen",
        "SoundEndsOnEmitterEnd",
        "SoundOnCreate",
        "SoundPersistent",
        "SoundsPlayWhileOffScreen",
        "VoiceOverOnCreate",
        "VoiceOverPersistent",
    ];
    let mut r = generate_list(base, 0, 50);
    r.extend(generate_list(MATERIAL_NAMES, 0, 5));
    r
}

fn get_group_names() -> Vec<String> {
    let base: &[&str] = &[
        "ExcludeAttachmentType",
        "KeywordsExcluded",
        "KeywordsIncluded",
        "KeywordsRequired",
        "Particle-ScaleAlongMovementVector",
        "SoundOnCreate",
        "SoundPersistent",
        "VoiceOverOnCreate",
        "VoiceOverPersistent",
        "dont-scroll-alpha-UV",
        "e-active",
        "e-alpharef",
        "e-beam-segments",
        "e-censor-policy",
        "e-disabled",
        "e-life",
        "e-life-scale",
        "e-linger",
        "e-local-orient",
        "e-period",
        "e-shape-name",
        "e-shape-scale",
        "e-shape-use-normal-for-birth",
        "e-soft-in-depth",
        "e-soft-out-depth",
        "e-soft-in-depth-delta",
        "e-soft-out-depth-delta",
        "e-timeoffset",
        "e-trail-cutoff",
        "e-trail-smoothing",
        "e-uvscroll",
        "e-uvscroll-mult",
        "flag-brighter-in-fow",
        "flag-disable-z",
        "flag-disable-y",
        "flag-groundlayer",
        "flag-ground-layer",
        "flag-force-animated-mesh-z-write",
        "flag-projected",
        "p-alphaslicerange",
        "p-animation",
        "p-backfaceon",
        "p-beammode",
        "p-bindtoemitter",
        "p-coloroffset",
        "p-colorscale",
        "p-colortype",
        "p-distortion-mode",
        "p-distortion-power",
        "p-falloff-texture",
        "p-fixedorbit",
        "p-fixedorbittype",
        "p-flexoffset",
        "p-flexscale",
        "p-followterrain",
        "p-frameRate",
        "p-frameRate-mult",
        "p-fresnel",
        "p-life-scale",
        "p-life-scale-offset",
        "p-life-scale-symX",
        "p-life-scale-symY",
        "p-life-scale-symZ",
        "p-linger",
        "p-local-orient",
        "p-lockedtoemitter",
        "p-mesh",
        "p-meshtex",
        "p-meshtex-mult",
        "p-normal-map",
        "p-numframes",
        "p-numframes-mult",
        "p-offsetbyheight",
        "p-offsetbyradius",
        "p-orientation",
        "p-projection-fading",
        "p-projection-y-range",
        "p-randomstartframe",
        "p-randomstartframe-mult",
        "p-reflection-fresnel",
        "p-reflection-map",
        "p-reflection-opacity-direct",
        "p-reflection-opacity-glancing",
        "p-rgba",
        "p-scalebias",
        "p-scalebyheight",
        "p-scalebyradius",
        "p-scaleupfromorigin",
        "p-shadow",
        "p-simpleorient",
        "p-skeleton",
        "p-skin",
        "p-startframe",
        "p-startframe-mult",
        "p-texdiv",
        "p-texdiv-mult",
        "p-texture",
        "p-texture-mode",
        "p-texture-mult",
        "p-texture-mult-mode",
        "p-texture-pixelate",
        "p-trailmode",
        "p-type",
        "p-uvmode",
        "p-uvparallax-scale",
        "p-uvscroll-alpha-mult",
        "p-uvscroll-no-alpha",
        "p-uvscroll-rgb",
        "p-uvscroll-rgb-clamp",
        "p-uvscroll-rgb-clamp-mult",
        "p-vec-velocity-minscale",
        "p-vec-velocity-scale",
        "p-vecalign",
        "p-xquadrot-on",
        "pass",
        "rendermode",
        "single-particle",
        "submesh-list",
        "teamcolor-correction",
        "uniformscale",
        "ChildParticleName",
        "ChildSpawnAtBone",
        "ChildEmitOnDeath",
        "p-childProb",
        "ChildParticleName%N%",
        "ChildSpawnAtBone%N%",
        "ChildEmitOnDeath%N%",
    ];
    let mut r = generate_list(base, 0, 10);
    r.extend(generate_list(MATERIAL_NAMES, 0, 5));
    r.extend(rand_color_amount(&s(&["e-rgba", "p-xrgba"])));
    r.extend(flex_names(&["p-scale", "p-scaleEmitOffset"]));
    r.extend(flex_rand_float(&["e-rate", "p-life", "p-rotvel"]));
    r.extend(flex_rand_vec2(&["e-uvoffset"]));
    r.extend(flex_rand_vec3(&["p-offset", "p-postoffset", "p-vel"]));
    r.extend(rand_color(&s(&[
        "e-censor-modulate",
        "p-fresnel-color",
        "p-reflection-fresnel-color",
    ])));
    r.extend(rand_float(&s(&[
        "e-color-modulate",
        "e-framerate",
        "p-bindtoemitter",
        "p-life",
        "p-quadrot",
        "p-rotvel",
        "p-scale",
        "p-xquadrot",
        "p-xscale",
        "e-rate",
    ])));
    r.extend(rand_vec2(&s(&[
        "e-ratebyvel",
        "e-uvoffset",
        "e-uvoffset-mult",
        "p-uvscroll-rgb",
        "p-uvscroll-rgb-mult",
    ])));
    r.extend(rand_vec3(&s(&[
        "Emitter-BirthRotationalAcceleration",
        "Particle-Acceleration",
        "Particle-Drag",
        "Particle-Velocity",
        "e-tilesize",
        "p-accel",
        "p-drag",
        "p-offset",
        "p-orbitvel",
        "p-postoffset",
        "p-quadrot",
        "p-rotvel",
        "p-scale",
        "p-vel",
        "p-worldaccel",
        "p-xquadrot",
        "p-xrgba-beam-bind-distance",
        "p-xscale",
    ])));
    r.extend(rand_float(&generate_list(&["e-rotation%N%"], 0, 10)));
    r.extend(generate_list(&["e-rotation%N%-axis"], 0, 10));
    r.extend(generate_list(PART_FIELD_NAMES, 1, 10));
    r.extend(PART_FLUID_NAMES.iter().map(|s| s.to_string()));
    r
}

fn get_field_names() -> Vec<String> {
    let mut r: Vec<String> = FIELD_NAMES_BASE.iter().map(|s| s.to_string()).collect();
    r.extend(rand_float(&s(&[
        "f-accel",
        "f-drag",
        "f-freq",
        "f-frequency",
        "f-period",
        "f-radius",
        "f-veldelta",
    ])));
    r.extend(rand_vec3(&s(&[
        "f-accel",
        "f-direction",
        "f-pos",
        "f-axisfrac",
    ])));
    r
}

fn get_fluid_names() -> Vec<String> {
    let base: &[&str] = &[
        "f-accel",
        "f-buoyancy",
        "f-denseforce",
        "f-diffusion",
        "f-dissipation",
        "f-life",
        "f-initdensity",
        "f-movement-x",
        "f-movement-y",
        "f-viscosity",
        "f-startkick",
        "f-rate",
        "f-rendersize",
        "f-jetdir%N%",
        "f-jetdirdiff%N%",
        "f-jetpos%N%",
        "f-jetspeed%N%",
    ];
    generate_list(base, 0, 4)
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Extract emitter group names from raw entries.
///
/// Looks for System[GroupPartN] entries whose values are strings — these
/// are the actual emitter names used as section keys for field hashing.
pub fn extract_group_names(entries: &[crate::types::RawEntry]) -> Vec<String> {
    let system_names = get_system_names();
    let system_section = vec!["System".to_string()];
    let system_hashes = build_hash_entries(&system_section, &system_names);

    let mut group_names = Vec::new();
    for (_, name, hash) in &system_hashes {
        // Match GroupPartN (not GroupPartNType or GroupPartNImportance)
        if !name.starts_with("GroupPart") || name.contains("Type") || name.contains("Importance") {
            continue;
        }
        if name.starts_with('\'') {
            continue;
        }

        for entry in entries {
            if entry.hash == *hash {
                if let crate::types::Value::String(ref s) = entry.value {
                    if !s.is_empty() && !group_names.contains(s) {
                        group_names.push(s.clone());
                    }
                }
            }
        }
    }
    group_names
}

/// Build a complete hash → `(section, field_name)` resolution map.
///
/// The `group_names` parameter should be the actual emitter names extracted
/// from the System section (via [`extract_group_names`]).
pub fn build_hash_map(group_names: &[String]) -> HashMap<u32, (String, String)> {
    let group_field_names = get_group_names();
    let field_names = get_field_names();
    let fluid_names = get_fluid_names();

    let mut map: HashMap<u32, (String, String)> = HashMap::new();

    let mut add = |sections: &[String], names: &[String]| {
        for (section, name, hash) in build_hash_entries(sections, names) {
            map.entry(hash).or_insert((section, name));
        }
    };

    // System section
    add(&["System".to_string()], &get_system_names());

    // Group sections with actual emitter names
    for gn in group_names {
        add(std::slice::from_ref(gn), &group_field_names);
    }

    // Also add GroupPartN as fallback sections
    let gpart_sections: Vec<String> = (0..50).map(|i| format!("GroupPart{}", i)).collect();
    add(&gpart_sections, &group_field_names);

    // Field sub-groups: use emitter names as sections for field-* entries
    // First extract field sub-group names from the data
    let field_subgroup_names: Vec<String> = generate_list(PART_FIELD_NAMES, 1, 10);
    for gn in group_names {
        // Look for field-accel-N etc. values in each emitter
        add(std::slice::from_ref(gn), &field_subgroup_names);
    }

    // Field and fluid entries use the sub-group names as sections
    // (these are the values of field-accel-N, field-drag-N, etc.)
    // We can't know these without the actual data, but we can pre-hash
    // common patterns. For a full match, the caller should re-resolve
    // after extracting field/fluid sub-group values.
    add(&gpart_sections, &field_names);
    add(&gpart_sections, &fluid_names);
    for gn in group_names {
        add(std::slice::from_ref(gn), &field_names);
        add(std::slice::from_ref(gn), &fluid_names);
    }

    map
}

/// Resolve a single hash using a pre-built map.
pub fn resolve_hash(hash: u32, map: &HashMap<u32, (String, String)>) -> Option<(&str, &str)> {
    map.get(&hash).map(|(s, f)| (s.as_str(), f.as_str()))
}

/// Convenience: build the hash map and resolve a list of raw entries,
/// extracting group names automatically from the entries.
pub fn resolve_entries(entries: &[crate::types::RawEntry]) -> HashMap<u32, (String, String)> {
    let group_names = extract_group_names(entries);
    build_hash_map(&group_names)
}

/// Build a reverse map: `(section, field_name)` → hash.
///
/// Used when writing binary from INI text (need to hash the field names back).
pub fn build_reverse_map(group_names: &[String]) -> HashMap<(String, String), u32> {
    let mut map = HashMap::new();
    let forward = build_hash_map(group_names);
    for (hash, (section, field)) in forward {
        map.entry((section, field)).or_insert(hash);
    }
    // Also allow direct computation for any section+field pair
    map
}

/// Compute the hash for a known section+field pair directly.
pub fn hash_section_field(section: &str, field: &str) -> u32 {
    section_field_hash(section, field)
}
