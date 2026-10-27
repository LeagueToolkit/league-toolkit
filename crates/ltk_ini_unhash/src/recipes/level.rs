//! The recipe of the files of a level.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    level_atmospheremutators::read(r);
    level_audio::read(r);
    level_clouds::read(r);
    level_deathtimes::read(r);
    level_envlighting::read(r);
    level_expcurve::read(r);
    level_gamma::read(r);
    level_graphics::read(r);
    level_info::read(r);
    level_items::read(r);
    level_mission::read(r);
    level_monsterdatatable::read(r);
    level_navgrid::read(r);
    level_neutraltimers::read(r);
    level_particles::read(r);
    level_perside::read(r);
    level_shadowsettings::read(r);
    level_statsprogression::read(r);
    level_terrain::read(r);
    level_vtex::read(r);
}

// LEVELS/*/AtmosphereMutators.inibin
mod level_atmospheremutators {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Atmosphere"] {
                seq string "Mutator" from 1;
            }
        );
    }
}

// LEVELS/*/Audio.inibin
mod level_audio {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn banks(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            int "NumOfSoundBanks";
            seq string "SoundBank" from 1;
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for n in [
                "Default", "GameMode_TUTORIAL", "Mutator_Bilgewater", "Mutator_DOOM",
                "Mutator_KingPoro",
            ] {
                [n] {
                    string
                        "AmbientEvent",
                        "ReverbPreset",
                        "ThemeMusic",
                        "ThemeMusic1",
                        "ThemeMusicTransition",
                        "ThemeMusicTransition1",
                        "VictoryMusic",
                        "DefeatMusic",
                        "VictoryBannerSound",
                        "DefeatBannerSound";
                }
            }
            ["SoundBanks"] {
                use banks();
                string "EventLookupTable";
            }
            for x in ["TUTORIAL", "KingPoro", "Bilgewater", "DOOM"] {
                ["AdditionalSoundBanks_" x] {
                    use banks();
                }
            }
        );
    }
}

// LEVELS/*/clouds.inibin
mod level_clouds {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn clouds(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "Texture";
            int "Tile";
            for n in ["SpeedU", "SpeedV"] {
                float n;
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for s in ["Clouds", "SandstormClouds"] {
                [s] {
                    use clouds();
                }
            }
        );
    }
}

// LEVELS/*/DeathTimes*.inibin
mod level_deathtimes {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn per_level(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            for i in 1.. while float ("Level" (if i < 10 { "0" } else { "" }) i) {
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for n in ["TimeDeadPerLevel", "TimeDeadPerLevelTutorial"] {
                [n] {
                    use per_level();
                }
            }
            ["DeathTimeScaling"] {
                int
                    "StartTime",
                    "IncrementTime";
                float
                    "PercentIncrease",
                    "PercentCap";
                if let n = int "NumberOfDeathScalingPoints" {
                    for i in (1..n + 1) {
                        int ("StartTime" i);
                        float ("PercentIncrease" i);
                    }
                }
            }
            ["DeathTimeSettings"] {
                int
                    "AllowDeathTimeMods",
                    "StartDeathTimerForZombies";
            }
            ["DeathWaveRespawn"] {
                int "WaveRespawnInterval";
            }
        );
    }
}

// LEVELS/*/envlighting.inibin
mod level_envlighting {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    fn colors(r: &mut Reader, s: Sec, prefix: Parts<'_>) {
        recipe!(r, s =>
            for c in ["red", "green", "blue"] {
                int (prefix "_" c);
            }
        );
    }

    fn light(r: &mut Reader, s: Sec, n: i64) {
        recipe!(r, s =>
            for i in (0..n) {
                use colors(name!(("Color" i)));
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["SkyLight"] {
                use light(3);
                float "Intensity";
                int "light_count";
            }
            ["SunLight"] {
                use light(2);
                float "Intensity";
                vec3 "Direction";
            }
            for n in ["SkyLightAO", "SunLightAO"] {
                [n] {
                    use light(2);
                    float "interp_pos";
                }
            }
            ["FoW"] {
                use colors(name!("Color"));
                float "Fade";
                string "Blend";
                int "Upscale";
            }
        );
    }
}

// LEVELS/*/ExpCurve*.inibin
mod level_expcurve {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for n in ["EXP", "EXPTutorial"] {
                [n] {
                    seq float "Level" from 2;
                }
            }
            ["ExpGrantedOnDeath"] {
                float
                    "BaseExpMultiple",
                    "LevelDifferenceExpMultiple",
                    "MinimumExpMultiple";
            }
            ["ExpGrantedOnDeathTable"] {
                seq float "Level" from 1;
                float
                    "BaseExpMultipleTableSystem",
                    "LevelDifferenceExpMultipleTableSystem",
                    "MinimumExpMultipleTableSystem",
                    "ExperimentalLevelDifferenceExpMultipleTableSystem",
                    "ExperimentalTeamRubberbandCoefficient",
                    "ExperimentalTeamRubberbandLogSteepness",
                    "ExperimentalTeamRubberbandMaximumPercent";
                int "ExperimentalTeamRubberbandMaxLevelGap";
            }
        );
    }
}

// LEVELS/*/Gamma.inibin
mod level_gamma {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Gamma"] {
                int "IsEnabled";
                for n in ["Gamma", "Brightness", "Contrast", "Level"] {
                    float n;
                }
            }
        );
    }
}

// LEVELS/*/Graphics.inibin
mod level_graphics {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["TextureQuality"] {
                for n in ["Default", "Effects", "Grass", "Terrain", "World"] {
                    float n;
                }
            }
            ["SimpleEnvironment"] {
                vec4 "HighlightColor";
            }
        );
    }
}

// DATA/Levels/*.inibin
mod level_info {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Data"] {
                int
                    "MapID",
                    "TotalPlayers";
                string
                    "DisplayName",
                    "Description";
                int
                    "PlatformEnabled",
                    "TutorialEnabled",
                    "TutorialOnly";
            }
        );
    }
}

// LEVELS/*/Items*.inibin
mod level_items {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn item_list(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            seq int "Item" from 0;
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["ItemInclusionList"] {
                use item_list();
            }
            ["UnpurchasableItemList"] {
                use item_list();
            }
            ["UnpurchasableItemListTutorial"] {
                use item_list();
            }
            for mode in [
                "ARAM", "ASCENSION", "FIRSTBLOOD", "KingPoro", "ODIN", "TUTORIAL", "URF",
                "NoSnowballItems",
            ] {
                ["UnpurchasableItemList_" mode] {
                    use item_list();
                }
            }
        );
    }
}

// LEVELS/*/Mission.inibin
mod level_mission {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["General"] {
                string
                    "MissionScript",
                    "StringID";
                float "MinionSpawnDelay";
                int
                    "AnimatedEndOfGame",
                    "RelativeColorization";
            }
            ["LoadScreen"] {
                string "ScreenToLoad";
            }
            ["Surrender"] {
                int
                    "MinAFKTimeForNoVote",
                    "MinTimeBetweenSurrenders",
                    "MinTimeUntilSurrenderAllowedSeconds",
                    "TimeBetweenSurrenderReminder",
                    "TimeDelayAfterSurrenderMessage",
                    "TimeOutForSurrenderSeconds";
                float "PercentageNeededForSurrender";
            }
            ["Equalize"] {
                int
                    "MinAFKTimeForNoVote",
                    "MinTimeBetweenEqualize",
                    "MinTimeUntilEqualizeAllowedSeconds",
                    "TimeBetweenEqualizeReminder",
                    "TimeDelayAfterEqualizeMessage",
                    "TimeOutForEqualizeSeconds";
                float "PercentageNeededForEqualize";
            }
            ["Experience"] {
                for i in 1..6 {
                    float ("PlayerMinionSplitXP" i);
                }
            }
            ["BehaviorTreeMetaData"] {
                string
                    "BehaviorTreeNodeLibraryCommon",
                    "BehaviorTreeNodeLibraryClient",
                    "BehaviorTreeNodeLibraryServer",
                    "ScriptSolution",
                    "ScriptBehaviorTrees";
            }
            for m in [
                "", "ARAM", "ASCENSION", "CLASSIC", "DARKSTAR", "FIRSTBLOOD", "GM1", "KINGPORO",
                "ODIN", "SIEGE", "TUTORIAL",
            ] {
                ["BehaviorTreeInstances" m] {
                    string
                        "BehaviorTreeDefinitions",
                        "ScriptSolution",
                        "ScriptBehaviorTrees";
                }
                ["ExtraSummonerItem" m] {
                    string
                        "Icon",
                        "Tooltip";
                    int "ItemID";
                }
            }
            ["BuffCountTracker"] {
                string "TeamBuffs";
                int "TeamBuffGloballyVisible";
                for i in 1..7 {
                    string ("TeamBuffs" i);
                    int ("TeamBuffGloballyVisible" i);
                }
            }
        );
    }
}

// LEVELS/*/MonsterDataTable*.inibin
mod level_monsterdatatable {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for stat in [
                "AbilityPower", "Armor", "AttackDamage", "Experience", "Gold", "Health",
                "MagicResist",
            ] {
                [stat] {
                    seq float "Level" from 0;
                }
            }
        );
    }
}

// LEVELS/*/NavGridEditor/*.inibin
mod level_navgrid {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for i in 0.. while ["Group" i] string "GroupName" {
                seq string "Value" from 0;
                seq vec3 "Color" from 0;
            }
        );
    }
}

// LEVELS/*/NeutralTimers*.inibin
mod level_neutraltimers {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    fn art(r: &mut Reader, s: Sec, name: Parts<'_>) {
        recipe!(r, s =>
            for v in ["", "_Colorblind", "_Default"] {
                string (name v);
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["General"] {
                string
                    "HudGroup",
                    "HudFrameHandle",
                    "TimerTooltip";
                int "TimerWarningThreshold";
                for i in 0..4 {
                    use art(name!(("Element" i)));
                }
            }
            for i in 0.. while ["Timer" i] string "TimerType" {
                for n in ["HudHandleIcon", "HudDeadHandleIcon", "DynamicBackground"] {
                    use art(name!(n));
                }
                string
                    "HudHandleText",
                    "TimerTooltip",
                    "TooltipName",
                    "TooltipRespawn",
                    "NicknameOrder",
                    "NicknameChaos",
                    "SourceTooltipBaseName";
                int "MinutesWidth";
                seq string "SourceIcon" from 0;
            }
        );
    }
}

// LEVELS/*/Particles*.inibin
mod level_particles {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["GlobalEffects"] {
                int "MaxIdentifier";
                for i in 0..48 {
                    string
                        ("EffectName" i),
                        ("FileName" i);
                }
                string
                    "MinionDeath",
                    "TurretDeath",
                    "OrderBarrackDeath",
                    "ChaosBarrackDeath",
                    "OrderHQDeath",
                    "ChaosHQDeath",
                    "StartHeroSpawn",
                    "HeroRespawn",
                    "FocusTarget",
                    "FountainHeal",
                    "CommandTarget",
                    "CommandGround",
                    "DefendTarget",
                    "ComeHere",
                    "OMW",
                    "Retreat",
                    "Danger",
                    "MIA",
                    "HeroKilledByHero",
                    "HeroKilledByNeutral",
                    "HeroKilledByPlayer",
                    "HeroKilledAssistedByPlayer",
                    "HeroAllyKilled",
                    "HeroEnemyKilled";
            }
            ["FootSteps"] {
                string "EmitterBone";
                int "EmitterRotation";
                for n in ["State", "Key", "EffectName"] {
                    seq string n from 0;
                }
            }
        );
    }
}

// LEVELS/*/PerSide.inibin
mod level_perside {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Camera"] {
                for team in ["Order", "Chaos"] {
                    for axis in ["X", "Z"] {
                        int (team axis "Offset");
                    }
                }
            }
        );
    }
}

// LEVELS/*/ShadowSettings.inibin
mod level_shadowsettings {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Shadow"] {
                float "ShadowBias";
                for n in ["ShadowFadeMinY", "ShadowFadeRange"] {
                    int n;
                }
            }
        );
    }
}

// LEVELS/*/StatsProgression.inibin
mod level_statsprogression {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["PerLevelStatsFactor"] {
                seq float "Level" from 1;
            }
        );
    }
}

// LEVELS/*/terrain*.inibin
mod level_terrain {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn blending(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            int "Enable";
            for i in 0..4 {
                float ("Layer" i "_scale");
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Environment"] {
                string
                    "Renderer",
                    "DevMark",
                    "NavmeshMaskTexture";
                int
                    "Lighting",
                    "DisablePointLights",
                    "ShadowBlur",
                    "ShadowDynamicOnly";
                vec3 "ShadowColor";
            }
            for n in ["Sun", "Sun_NightmareSRVisual"] {
                [n] {
                    vec3
                        "SunDir",
                        "SunLightColor",
                        "AmbientLightColor";
                }
            }
            ["Fog"] {
                int
                    "Enable",
                    "UseOpacityMap";
                int
                    "Red",
                    "Green",
                    "Blue";
                float
                    "Opacity",
                    "Min_height",
                    "Max_height",
                    "Min_opaque_height",
                    "Max_opaque_height",
                    "Min_radius",
                    "Max_radius";
            }
            ["FoW"] {
                for c in ["red", "green", "blue"] {
                    int ("Color_" c);
                }
                float
                    "baseMapBrightness",
                    "BlurKernelSigma",
                    "EdgeEnhancement",
                    "EdgeTintPoint",
                    "FadeToFoggedValue";
                int
                    "FadeStart",
                    "FadeFinish";
                int
                    "FadeToFogged",
                    "edgeReboundPlateau",
                    "Upscale";
                vec4 "SandstormAnimate";
            }
            for n in ["HeightBlending", "TerrainBlending"] {
                [n] {
                    use blending();
                }
            }
            ["Cheat"] {
                float
                    "FOV",
                    "ZoomScale";
            }
            ["ColorMap"] {
                int "Enable";
                for i in 0..8 {
                    for c in ["Hue", "Sat", "Val"] {
                        int ("pal" i "_" c);
                    }
                }
            }
            ["Editor"] {
                int
                    "undo_count",
                    "pressure_min",
                    "pressure_max",
                    "pressure_power";
                int
                    "tablet_enable",
                    "continuous_accum";
            }
        );
    }
}

// LEVELS/*/vtex.inibin
mod level_vtex {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["basic"] {
                int "render";
            }
        );
    }
}
