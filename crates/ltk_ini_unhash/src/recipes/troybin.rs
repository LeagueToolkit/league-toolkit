//! The recipe of `.troybin` particle systems.
//!
//! `[System]` names the emitter sections in `GroupPart1`, `GroupPart2`, and so on.
//! `GroupPartNType` selects the Simple or the Complex reader of an emitter. An emitter names its
//! field sections (`field-<kind>-1` to `field-<kind>-8`) and its fluid section (`fluid-params`).
//! A Simple `GroupPart` of old builds names a group, whose `Emitter1` to `Emitter8` name the
//! emitters.

use crate::reader::{Parts, Reader, Sec};
use crate::recipes::{name, recipe};
use crate::Ty;

pub(crate) fn read(r: &mut Reader) {
    let sys = r.section(name!("System"));
    recipe!(r, sys =>
        string
            "AudioFlexValueParameterName",
            "AudioParameterFlexID",
            "AudioParameterTimeScaledDuration",
            "DrawingLayer";
        float
            "group-vis",
            "group-scale-cap",
            "build-up-time",
            "SelfIllumination";
        int
            "SimulateWhileOffScreen",
            "SimulateOncePerFrame",
            "SimulateEveryFrame",
            "SoundEndsOnEmitterEnd",
            "SoundEndsOnEmmiterEnd",
            "SoundsPlayWhileOffScreen",
            "PersistThruDeath",
            "PersistThruRevive",
            "KeepOrientationAfterSpellCast",
            "scale-with-attached-bone";
        key
            "hud-layer-dimension",
            "hud-layer-anchor-world";
        vec3
            "system-scale",
            "system-translation";
        use sounds();
        use material_overrides();
        for i in 1.. while string ("assetmap" i) {}
        stale {
            for i in 0..100 {
                for k in ["Offset", "Rotation", "Scale"] {
                    vec3 ("Override-" k i);
                }
            }
        }
    );
    group_parts(r, sys);
}

/// The client reads `GroupPart1` to `GroupPart8`, and the parts after them up to the first
/// missing one. A file also holds parts that the client never reads: `GroupPart0`, the parts after
/// a gap, the keys of the other reader.
fn group_parts(r: &mut Reader, sys: Sec) {
    let mut reading = true;
    for i in 0..100 {
        let name = r.stale_unless(reading && i > 0, |r| r.string(sys, name!(("GroupPart" i))));
        if name.is_none() && i > 8 {
            reading = false;
        }
        r.stale_unless(reading && i > 0 && name.is_some(), |r| {
            r.string(sys, name!(("GroupPart" i "Importance")));
            let simple = r
                .string(sys, name!(("GroupPart" i "Type")))
                .is_some_and(|ty| ty.eq_ignore_ascii_case("Simple"));
            // 0-based, unlike GroupPart, and only for Complex parts.
            r.stale_unless(!simple, |r| {
                for k in ["Offset", "Rotation", "Scale"] {
                    r.vec3(sys, name!(("Override-" k (i - 1))));
                }
            });
            if let Some(name) = &name {
                let e = r.section(name!(name));
                if simple {
                    simple_group(r, e);
                    r.stale(|r| complex_emitter(r, e));
                } else {
                    complex_emitter(r, e);
                    r.stale(|r| simple_emitter(r, e));
                }
            }
        });
        // A part switched off with a comment prefix: the emitter it names is in the file.
        if name.is_none() {
            r.stale(|r| {
                let off = ["'GroupPart", "''GroupPart", "' GroupPart"]
                    .into_iter()
                    .find_map(|prefix| r.string(sys, name!((prefix i))));
                if let Some(off) = off {
                    let e = r.section(name!(off));
                    simple_group(r, e);
                    complex_emitter(r, e);
                }
            });
        }
        // A part that only other versions of the file list.
        r.stale(|r| {
            for prefix in ["", "'"] {
                for other in r.history(sys, name!((prefix "GroupPart" i))) {
                    if name
                        .as_deref()
                        .is_some_and(|name| name.eq_ignore_ascii_case(other))
                    {
                        continue;
                    }
                    let e = r.section(name!(other));
                    if EMITTER_KEYS.iter().any(|key| r.has(e, name!(key))) {
                        simple_group(r, e);
                        complex_emitter(r, e);
                    }
                }
            }
        });
    }
}

/// Keys that most emitters have.
const EMITTER_KEYS: &[&str] = &[
    "e-rate",
    "p-life",
    "e-life",
    "rendermode",
    "p-texture",
    "p-scale",
    "pass",
    "p-type",
];

// --- animated values ------------------------------------------------------------------------------

/// Returns the type of a point of the curve of a value of type `ty`: a time and the value.
fn curve(ty: Ty) -> Ty {
    match ty {
        Ty::Float => Ty::Vec2,
        Ty::Vec2 => Ty::Vec3,
        Ty::Vec3 => Ty::Vec4,
        _ => Ty::Vec5,
    }
}

/// Returns the suffixes of the probability tables of the components of a value of type `ty`.
fn components(ty: Ty) -> &'static [&'static str] {
    match ty {
        Ty::Float => &["X", ""],
        Ty::Vec2 => &["X", "Y"],
        Ty::Vec3 => &["X", "Y", "Z"],
        _ => &["R", "G", "B", "A"],
    }
}

/// Reads the probability table `t`: one float, or the points `t1`, `t2`, and so on. The client
/// reads the points only without the float, and reads no point 0.
fn table(r: &mut Reader, s: Sec, t: Parts<'_>) {
    if r.float(s, t).is_some() {
        r.stale(|r| r.seq(s, t, 1, Some(Ty::Vec2)));
    } else {
        r.seq(s, t, 1, Some(Ty::Vec2));
    }
    r.stale(|r| r.vec2(s, name!((t 0))));
}

/// Reads the animated value `name`: the constant `name`, the curve points `name1`, `name2`, and so
/// on, and a probability table per component. Returns `true` if the file has the constant or a
/// curve point.
fn anim(r: &mut Reader, s: Sec, name: Parts<'_>, ty: Ty) -> bool {
    let constant = r.typed(s, name, Some(ty));
    let found = r.seq(s, name, 1, Some(curve(ty))).is_some() || constant;
    for c in components(ty) {
        table(r, s, name!((name c "P")));
    }
    r.stale(|r| r.typed(s, name!((name 0)), Some(curve(ty))));
    // A constant with a comment prefix still has its tables in the file.
    r.has(s, name);
    found
}

fn anims(r: &mut Reader, s: Sec, ty: Ty, names: &[&str]) {
    for name in names {
        anim(r, s, name!(name), ty);
    }
}

/// Reads `name_flex0` to `name_flex2` with `read`. The client stops at the first one that the file
/// has.
fn flex(r: &mut Reader, name: Parts<'_>, mut read: impl FnMut(&mut Reader, Parts<'_>) -> bool) {
    let mut done = false;
    for i in 0..3 {
        let flex = name!((name "_flex" i));
        if done {
            r.stale(|r| read(r, flex));
        } else {
            done = read(r, flex);
        }
    }
}

fn flex_anim(r: &mut Reader, s: Sec, name: Parts<'_>, ty: Ty) {
    flex(r, name, |r, flex| anim(r, s, flex, ty));
}

fn flex_float(r: &mut Reader, s: Sec, name: Parts<'_>) {
    flex(r, name, |r, flex| r.float(s, flex).is_some());
}

// --- shared by [System] and emitters --------------------------------------------------------------

fn sounds(r: &mut Reader, s: Sec) {
    recipe!(r, s =>
        string
            "SoundOnCreate",
            "VoiceOverOnCreate",
            "SoundPersistent",
            "VoiceOverPersistent";
    );
}

/// The transition map is read only with an override, and its sample only with a map.
fn material_overrides(r: &mut Reader, s: Sec) {
    let mut any = false;
    for i in 0..4 {
        any |= r.typed(s, name!(("MaterialOverride" i "SubMesh")), Some(Ty::String));
        any |= r.typed(s, name!(("MaterialOverride" i "Texture")), Some(Ty::String));
        recipe!(r, s =>
            string ("MaterialOverride" i "GlossTexture");
            int
                ("MaterialOverride" i "Priority"),
                ("MaterialOverride" i "FixedAlphaScrolling"),
                ("MaterialOverride" i "BlendMode");
            vec2 ("MaterialOverride" i "UVScroll");
        );
    }
    let trans = r.stale_unless(any, |r| {
        r.typed(s, name!("MaterialOverrideTransMap"), Some(Ty::String))
    });
    recipe!(r, s =>
        stale unless (any) {
            int "MaterialOverrideTransSource";
        }
        stale unless (any && trans) {
            int "p-trans-sample";
        }
    );
}

// --- fields and fluids ----------------------------------------------------------------------------

fn field(r: &mut Reader, f: Sec, kind: &str) {
    match kind {
        "accel" => {
            r.int(f, name!("f-localspace"));
            anim(r, f, name!("f-accel"), Ty::Vec3);
        }
        "attract" => {
            anim(r, f, name!("f-pos"), Ty::Vec3);
            anims(r, f, Ty::Float, &["f-radius", "f-accel"]);
        }
        "drag" => {
            anim(r, f, name!("f-pos"), Ty::Vec3);
            anims(r, f, Ty::Float, &["f-radius", "f-drag"]);
        }
        "noise" => {
            anim(r, f, name!("f-pos"), Ty::Vec3);
            anims(
                r,
                f,
                Ty::Float,
                &["f-radius", "f-period", "f-frequency", "f-veldelta"],
            );
            r.vec3(f, name!("f-axisfrac"));
        }
        _ => {
            r.int(f, name!("f-localspace"));
            anim(r, f, name!("f-direction"), Ty::Vec3);
        }
    }
    // The keys of the other field kinds.
    r.stale(|r| {
        r.int(f, name!("f-localspace"));
        anims(r, f, Ty::Vec3, &["f-pos", "f-accel", "f-direction"]);
        anims(
            r,
            f,
            Ty::Float,
            &[
                "f-radius",
                "f-accel",
                "f-drag",
                "f-period",
                "f-frequency",
                "f-veldelta",
            ],
        );
        r.vec3(f, name!("f-axisfrac"));
    });
}

fn fields(r: &mut Reader, e: Sec) {
    for kind in ["accel", "attract", "drag", "noise", "orbit"] {
        for i in 1..9 {
            if let Some(name) = r.string(e, name!(("field-" kind "-" i))) {
                let f = r.section(name!(name));
                field(r, f, kind);
            }
        }
    }
}

fn fluid(r: &mut Reader, f: Sec) {
    recipe!(r, f =>
        float
            "f-viscosity",
            "f-diffusion",
            "f-buoyancy",
            "f-dissipation",
            "f-startkick",
            "f-denseforce",
            "f-life",
            "f-rate";
        vec2 "f-accel";
        vec3
            "f-movement-x",
            "f-movement-y";
        for i in 1..4 {
            vec2
                ("f-jetpos" i),
                ("f-jetspeed" i);
            float
                ("f-jetdir" i),
                ("f-jetdirdiff" i);
        }
        string "f-initdensity";
        int "f-rendersize";
    );
}

// --- emitters -------------------------------------------------------------------------------------

/// The keys of both readers.
fn emitter_common(r: &mut Reader, e: Sec) {
    recipe!(r, e =>
        int
            "e-disabled",
            "e-censor-policy",
            "e-spectator-policy";
        string
            "ExcludeAttachmentType",
            "KeywordsExcluded",
            "KeywordsRequired",
            "KeywordsIncluded",
            "p-texture-mode",
            "p-texture",
            "p-rgba",
            "p-falloff-texture",
            "p-normal-map",
            "p-colortype";
        int
            "rendermode",
            "pass",
            "e-alpharef",
            "p-backfaceon",
            "p-distortion-mode",
            "flag-disable-z",
            "flag-projected",
            "teamcolor-correction",
            "flag-brighter-in-fow",
            "single-particle",
            "disable-fow",
            "e-particles-share-random",
            "e-sort-emitters-by-position",
            "p-usenavmeshmask",
            "dont-scroll-alpha-UV";
        float
            "p-distortion-power",
            "p-alphaslicerange",
            "e-soft-in-depth",
            "e-soft-out-depth",
            "e-soft-in-depth-delta",
            "e-soft-out-depth-delta",
            "e-life",
            "e-timeoffset",
            "e-active",
            "e-period",
            "p-linger",
            "e-linger",
            "p-offsetbyheight",
            "p-scalebyheight",
            "p-offsetbyradius",
            "p-scalebyradius";
        vec2
            "p-texdiv",
            "p-colorscale",
            "p-coloroffset",
            "e-depthbiasfactors";
        key
            "e-local-orient",
            "p-texture-pixelate",
            "p-flexoffset",
            "p-flexscale",
            "p-uvscroll-rgb-clamp",
            "e-life-scale",
            "p-life-scale";
        // a typo of `pass`
        stale {
            int "pas";
        }
        use flex_float(name!("p-scaleEmitOffset"));
        use flex_float(name!("p-scale"));
        use sounds();
        use material_overrides();
        if let f = string "fluid-params" {
            [f] {
                use fluid();
            }
        }
        use fields();
    );
    for (name, ty) in [
        ("p-life", Ty::Float),
        ("e-rate", Ty::Float),
        ("p-postoffset", Ty::Vec3),
        ("p-offset", Ty::Vec3),
        ("p-vel", Ty::Vec3),
    ] {
        anim(r, e, name!(name), ty);
        flex_anim(r, e, name!(name), ty);
    }
    anims(r, e, Ty::Vec4, &["e-color-modulate", "e-censor-modulate"]);
    // e-rotation1 to e-rotation9, each with an axis. The client stops at the first one that the
    // file has not.
    let mut rotating = true;
    for i in 1..10 {
        let rotation = name!(("e-rotation" i));
        if rotating {
            rotating = anim(r, e, rotation, Ty::Float);
        } else {
            r.stale(|r| anim(r, e, rotation, Ty::Float));
        }
        r.stale_unless(rotating, |r| r.vec3(e, name!(("e-rotation" i "-axis"))));
    }
}

/// The particle half of a Complex emitter, in the same section.
fn particle(r: &mut Reader, e: Sec) {
    recipe!(r, e =>
        int
            "p-startframe",
            "p-numframes",
            "p-type",
            "p-uvmode",
            "p-trailmode",
            "p-beammode",
            "p-randomstartframe",
            "p-shadow",
            "p-followterrain",
            "p-groundlayer",
            "p-lock-mesh-to-attachment";
        string
            "p-meshtex",
            "p-skin",
            "p-skeleton",
            "p-animation",
            "p-mesh",
            "e-shape-name";
        key
            "p-startframe-mult",
            "p-numframes-mult",
            "p-framerate",
            "p-framerate-mult",
            "p-randomstartframe-mult",
            "e-shape-scale",
            "e-shape-use-normal-for-birth",
            "p-meshtex-mult",
            "p-projection-y-range",
            "p-projection-fading",
            "p-uvscroll-alpha-mult",
            "p-uvscroll-rgb-clamp-mult",
            "p-xquadrot-on",
            "p-vecalign",
            "uniformscale",
            "p-vec-velocity-scale",
            "p-vec-velocity-minscale",
            "p-local-orient",
            "p-uvscroll-no-alpha",
            "p-uvparallax-scale";
        vec3
            "p-orientation",
            "p-beam-localspace-source-offset",
            "p-beam-localspace-target-offset";
        vec2
            "p-uvtransform-center",
            "p-uvtransform-center-mult";
        // children name other particle systems, not sections
        int "ChildEmitOnDeath";
        string "ChildParticleName";
    );
    anims(
        r,
        e,
        Ty::Vec2,
        &[
            "p-uvscroll-rgb",
            "p-uvscroll-rgb-mult",
            "p-life-uvscroll-rgb",
            "p-uvscale-rgb",
            "p-life-uvscroll-rgb-mult",
            "p-uvscale-rgb-mult",
        ],
    );
    flex_anim(r, e, name!("p-uvscroll-rgb"), Ty::Vec2);
    flex_anim(r, e, name!("p-uvscroll-rgb-mult"), Ty::Vec2);
    anims(
        r,
        e,
        Ty::Float,
        &[
            "p-uvrotate-rate-rgb",
            "p-uvrotation-rgb",
            "p-life-uvrotate-rate-rgb",
            "p-uvrotate-rate-rgb-mult",
            "p-uvrotation-rgb-mult",
            "p-life-uvrotate-rate-rgb-mult",
            "p-bindtoemitter",
        ],
    );
    anims(
        r,
        e,
        Ty::Vec3,
        &[
            "Particle-Velocity",
            "Particle-Acceleration",
            "p-worldaccel",
            "p-xscale",
            "Particle-Drag",
            "p-xquadrot",
        ],
    );
    anims(r, e, Ty::Vec4, &["p-xrgba", "p-xrgba-beam-bind-distance"]);
    // ChildSpawnAtBone<i> only with an unnumbered ChildSpawnAtBone.
    let bone = r.typed(e, name!("ChildSpawnAtBone"), Some(Ty::String));
    for i in 2.. {
        r.stale_unless(bone, |r| r.string(e, name!(("ChildSpawnAtBone" i))));
        if r.string(e, name!(("ChildParticleName" i))).is_none() {
            break;
        }
    }
    anim(r, e, name!("p-childProb"), Ty::Float);
}

fn complex_emitter(r: &mut Reader, e: Sec) {
    emitter_common(r, e);
    recipe!(r, e =>
        string
            "p-texture-mult-mode",
            "submesh-list",
            "submesh-list-always-draw",
            "p-texture-mult",
            "p-reflection-map";
        key
            "flag-force-animated-mesh-z-write",
            "p-texdiv-mult",
            "p-reflection-opacity-direct",
            "p-reflection-opacity-glancing",
            "p-reflection-fresnel",
            "p-fresnel",
            "e-trail-cutoff",
            "e-trail-max-added-per-frame",
            "e-trail-smoothing",
            "e-beam-segments",
            "e-stencil-mode",
            "e-stencil-ref";
        vec3 "p-life-scale-offset";
        float
            "p-life-scale-symX",
            "p-life-scale-symY",
            "p-life-scale-symZ";
        vec2
            "e-uvscroll",
            "e-uvscroll-mult";
        key
            "p-alpha-erosion-map",
            "p-alpha-erosion-map-mode",
            "p-alpha-erosion-drive-source",
            "p-alpha-erosion-feather-in",
            "p-alpha-erosion-feather-out",
            "p-alpha-erosion-slice-width",
            "p-palette-texture",
            "p-palette-count",
            "e-surflet-skin",
            "e-surflet-skel",
            "e-surflet-anim",
            "e-surflet-scale",
            "e-surflet-max-joint-weights",
            "e-surflet-use-avatar-pose",
            "e-surflet-use-normal-for-birth";
    );
    anims(
        r,
        e,
        Ty::Vec4,
        &["p-reflection-fresnel-color", "p-fresnel-color", "e-rgba"],
    );
    anims(
        r,
        e,
        Ty::Vec2,
        &["e-ratebyvel", "e-uvoffset", "e-uvoffset-mult"],
    );
    flex_anim(r, e, name!("e-uvoffset"), Ty::Vec2);
    anims(
        r,
        e,
        Ty::Vec3,
        &[
            "p-quadrot",
            "p-scale",
            "p-accel",
            "p-rotvel",
            "Emitter-BirthRotationalAcceleration",
            "p-drag",
            "p-orbitvel",
            "e-tilesize",
        ],
    );
    anims(r, e, Ty::Float, &["e-framerate", "p-alpha-erosion-drive"]);
    anims(
        r,
        e,
        Ty::Vec4,
        &["p-alpha-erosion-map-mixer", "p-palette-src-mix"],
    );
    anim(r, e, name!("p-palette-selector"), Ty::Vec3);
    particle(r, e);
}

/// The names of the particle half, read with other types.
fn simple_emitter(r: &mut Reader, e: Sec) {
    emitter_common(r, e);
    // p-type 3 is a skinned mesh with p-skin, otherwise a static p-mesh. p-type 7 is a projection.
    let ptype = r.int(e, name!("p-type"));
    let skin = r.stale_unless(ptype == Some(3), |r| {
        r.typed(e, name!("p-skin"), Some(Ty::String))
    });
    let skinned = ptype == Some(3) && skin;
    recipe!(r, e =>
        int
            "p-startframe",
            "p-numframes",
            "p-uvmode",
            "p-shadow",
            "p-followterrain",
            "p-groundlayer",
            "p-randomstartframe",
            "p-fixedorbittype",
            "p-simpleorient";
        stale unless (skinned) {
            string
                "p-skeleton",
                "p-animation",
                "p-weight";
        }
        stale unless (!skinned) {
            string "p-mesh";
        }
        stale unless (ptype == Some(7)) {
            key
                "p-projection-y-range",
                "p-projection-fading";
        }
        key
            "p-frameRate",
            "p-uvscroll-rgb-clamp",
            "p-vecalign",
            "p-xquadrot-on",
            "p-local-orient",
            "p-uvscroll-no-alpha",
            "Particle-ScaleAlongMovementVector",
            "p-fixedorbit",
            "p-lockedtoemitter",
            "p-scaleupfromorigin";
        vec2
            "p-uvscroll-rgb",
            "p-bindtoemitter",
            "p-scalebias";
    );
    anims(
        r,
        e,
        Ty::Float,
        &["p-xscale", "p-xquadrot", "p-quadrot", "p-scale", "p-rotvel"],
    );
    flex_anim(r, e, name!("p-rotvel"), Ty::Float);
}

/// A Simple part of old builds: a group of up to 8 simple emitters, or an emitter itself.
fn simple_group(r: &mut Reader, g: Sec) {
    simple_emitter(r, g);
    for i in 1..9 {
        if let Some(name) = r.string(g, name!(("Emitter" i))) {
            let e = r.section(name!(name));
            simple_emitter(r, e);
        }
    }
}
