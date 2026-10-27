//! The recipe of character files: units, skins, materials, shop events and recommended items.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    unit::read(r);
    evo::read(r);
    material::read(r);
    recitems::read(r);
}

// DATA/Characters/<c>/<c>.inibin
// DATA/**/Characters/<c>/<c>.inibin
// DATA/Characters/*/<c>/<c>.inibin
// DATA/Characters/<c>_OVERRIDE/<c>.inibin
// DATA/Characters/<c>/Spells/<c>.inibin
// DATA/Characters/Sion/SionBerserk.inibin
// DATA/Characters/Sion/SionMax.inibin
// DATA/Characters/Sion/SionPassive.inibin
// DATA/Characters/*/Skins/*/*.inibin
// not DATA/**/*.dds.inibin
//
// The client reads a unit file and a skin file with one loader. A skin file holds a subset of
// the sections of a unit file, and more MeshSkin keys. DATA/Characters/CorkiBomb/skins/base/
// Base.inibin is a unit file in a skin folder.
mod unit {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    fn attack(r: &mut Reader, d: Sec, a: Parts<'_>) {
        recipe!(r, d =>
            string a;
            float (a "_Probability");
            for k in [
                "AttackCastTime", "AttackTotalTime", "AttackDelayOffsetPercent",
                "AttackDelayCastOffsetPercent", "AttackDelayCastOffsetPercentAttackSpeedRatio",
            ] {
                float (a "_" k);
            }
        );
    }

    fn mesh_skin(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "SimpleSkin",
                "Skeleton",
                "Texture",
                "TextureLow",
                "Animations",
                "Body",
                "Weight",
                "ChampionSkinName",
                "EmissiveTexture",
                "GlossTexture",
                "GlossMap",
                "ReflectionMap",
                "ExtraCharacterPreloads",
                "SubmeshesToHide",
                "SubmeshMouseOversToHide",
                "SubmeshShadowsToHide",
                "Metadata",
                "IconAvatar",
                "VintageCutoffDate",
                "ParticleOverride_DeathParticle",
                "ParticleOverride_ChampionKillDeathParticle",
                "ArmorMaterial",
                "SkinClassification",
                "SkinAudioNameOverride",
                "VOOverride";
            float
                "SkinScale",
                "SelfIllumination",
                "GlowFactor",
                "BrushAlphaOverride",
                "YOffset",
                "XOffset",
                "Fresnel",
                "FresnelRed",
                "FresnelGreen",
                "FresnelBlue",
                "ReflectionFresnel",
                "ReflectionFresnelRed",
                "ReflectionFresnelGreen",
                "ReflectionFresnelBlue",
                "ReflectionOpacityDirect",
                "ReflectionOpacityGlancing";
            int
                "AttributeFlags",
                "CastShadows",
                "IsOpaque",
                "UsesSkinVO",
                "SkipVOOverride",
                "UseChildAnimationOverride";
            int
                "ChampionSkinID",
                "MaterialOverridePriority",
                "MaxNumBlendWeights",
                "ParallaxOffset";
            vec3 "OverrideBoundingBox";
            for i in 0..4 {
                for k in ["SubMesh", "Texture", "GlossTexture", "ReflectionMap"] {
                    string ("MaterialOverride" i k);
                }
            }
            seq string "ThemeMusic" from 0;
            stale {
                float
                    "Self_Illumination",
                    "ReflectionFresnlBlue";
                // Keys of other sections pasted into MeshSkin: 1 file of the corpus holds
                // GameplayCollisionRadius, none holds HPPerTick or WorldOffset.
                float "GameplayCollisionRadius";
                int "HPPerTick";
                vec3 "WorldOffset";
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Data"] {
                int
                    "IsEpic",
                    "IsElite",
                    "DrawPARLikeHealth",
                    "IsMelee",
                    "NeverRender",
                    "ServerOnly",
                    "NoAutoAttack",
                    "NoHealthBar",
                    "ShouldFaceTarget",
                    "PARDisplayThroughDeath",
                    "AllowPetControl",
                    "SkipDrawOutline",
                    "UseChampionVisibility",
                    "TriggersOrderAcknowledgementVO",
                    "FireworksEnabled",
                    "DisableUltReadySounds",
                    "IsImportantBotTarget",
                    "DisableGlobalDeathEffect",
                    "DisableAggroIndicator",
                    "SequentialAutoAttacks",
                    "DisableContinuousTargetFacing",
                    "Immobile",
                    "UseRingIconForKillCallout",
                    "RecordAsWard",
                    "LocalGoldSplitWithLastHitter",
                    "AlwaysVisible",
                    "BotEnabled",
                    "BotEnabledMM",
                    "CastShadows",
                    "DoesNotGiveMinionScore",
                    "PARHasRegenText",
                    "PlatformEnabled",
                    "SkipDrawToonInking",
                    "UseRiotRelationships",
                    "CS_easy",
                    "CS_medium",
                    "CS_hard",
                    "SR_intro",
                    "SR_easy",
                    "SR_medium",
                    "SR_hard",
                    "TT_easy",
                    "TT_medium";
                float
                    "PostAttackMoveDelay",
                    "ChasingAttackRangePercent",
                    "BaseSpellEffectiveness",
                    "LevelSpellEffectiveness",
                    "BaseHP",
                    "BaseMP",
                    "HPPerLevel",
                    "MPPerLevel",
                    "HPRegenPerLevel",
                    "MPRegenPerLevel",
                    "BaseStaticHPRegen",
                    "BaseFactorHPRegen",
                    "BaseStaticMPRegen",
                    "BaseFactorMPRegen",
                    "BaseDamage",
                    "DamagePerLevel",
                    "BaseAbilityPower",
                    "AbilityPowerIncPerLevel",
                    "Armor",
                    "ArmorPerLevel",
                    "SpellBlock",
                    "SpellBlockPerLevel",
                    "BaseDodge",
                    "LevelDodge",
                    "BaseMissChance",
                    "BaseCritChance",
                    "CritPerLevel",
                    "CritDamageBonus",
                    "MoveSpeed",
                    "AttackRange",
                    "AttackSpeed",
                    "AttackAutoInterruptPercent",
                    "AcquisitionRange",
                    "FirstAcquisitionRange",
                    "WakeUpRange",
                    "AttackSpeedPerLevel",
                    "TowerTargetingPriorityBoost",
                    "DeathTime",
                    "ExpGivenOnDeath",
                    "GoldGivenOnDeath",
                    "GoldRadius",
                    "ExperienceRadius",
                    "DeathEventListeningRadius",
                    "LocalGoldGivenOnDeath",
                    "LocalExpGivenOnDeath",
                    "GlobalGoldGivenOnDeath",
                    "GlobalExpGivenOnDeath",
                    "SoulGivenOnDeath",
                    "Significance",
                    "PerceptionBubbleRadius",
                    "HitFxScale",
                    "SelectionHeight",
                    "SelectionRadius",
                    "PathfindingCollisionRadius",
                    "GameplayCollisionRadius",
                    "OccludedUnitSelectableDistance",
                    "OutlineBBoxExpansion",
                    "UntargetableSpawnTime",
                    "DelayCastOffsetPercent",
                    "DelayTotalTimePercent",
                    "HoverIndicatorRadius",
                    "HoverLineIndicatorWidth",
                    "BaseAttackSpeedBonus";
                int
                    "ChampionId",
                    "MonsterDataTableId",
                    "AttackRank",
                    "DefenseRank",
                    "MagicRank",
                    "DifficultyRank",
                    "PARIncrements",
                    "PARMaxSegments";
                string
                    "PARType",
                    "AssetCategory",
                    "Name",
                    "Description",
                    "EnemyTooltip",
                    "FriendlyTooltip",
                    "Classification",
                    "Metadata",
                    "Roles",
                    "SearchTags",
                    "SearchTagsSecondary",
                    "UnitTags",
                    "PARColor",
                    "PARFadeColor",
                    "PARNameString",
                    "CharAudioNameOverride",
                    "ArmorMaterial",
                    "CritMaterial",
                    "WeaponMaterial",
                    "WeaponMaterialCrit",
                    "JointForAnimAdjustedSelection",
                    "HoverIndicatorTextureName",
                    "HoverLineIndicatorBaseTextureName",
                    "HoverLineIndicatorTargetTextureName",
                    "CriticalAttack",
                    "PassiveSpell";
                vec4 "MaxLevels";
                float
                    "Map8_BaseMP",
                    "Map10_BaseMP",
                    "Map10_ArmorPerLevel";
                for i in 1..5 {
                    string
                        ("Spell" i),
                        ("Spell" i "Desc"),
                        ("Spell" i "DisplayName"),
                        ("SpellsUpLevels" i);
                }
                for i in 1..17 {
                    string ("ExtraSpell" i);
                }
                for i in 1..3 {
                    string ("BotDefaultSpell" i);
                }
                use attack(name!("BaseAttack"));
                float
                    "AttackCastTime",
                    "AttackTotalTime",
                    "AttackDelayOffsetPercent",
                    "AttackDelayCastOffsetPercent",
                    "AttackDelayCastOffsetPercentAttackSpeedRatio";
                use attack(name!("CritAttack"));
                for i in 1..9 {
                    use attack(name!(("ExtraAttack" i)));
                    use attack(name!(("ExtraCritAttack" i)));
                }
                for i in 1..4 {
                    use attack(name!(("CritAttack" i)));
                }
                float
                    "CritAttackDelayOffsetPercent",
                    "CritAttackDelayCastOffsetPercent",
                    "CritAttack_AttackCastDelayOffsetPercent";
                for i in 1..5 {
                    int
                        ("Passive" i),
                        ("Passive" i "NumEffects");
                    float ("Passive" i "Range");
                    string
                        ("Passive" i "Name"),
                        ("Passive" i "Desc"),
                        ("Passive" i "Icon"),
                        ("Passive" i "LuaName");
                    for j in 1..7 {
                        int ("Passive" i "Level" j);
                    }
                    for j in 1..6 {
                        float ("Passive" i "Effect" j);
                    }
                }
                string "PassLev1Desc";
                for j in 1..7 {
                    string ("PassLev1Desc" j);
                }
                for i in 1..4 {
                    string
                        ("Lore" i),
                        ("Tips" i),
                        ("WeaponMaterial" i),
                        ("PurchaseIdentity" i);
                }
                stale {
                    string
                        "ExraSpell4",
                        "ExreSpell4";
                    float
                        "CritAttack_Probabilty",
                        "CritAttack1_Probabilty",
                        "BaseStaticsMPRegen";
                }
            }
            ["MeshSkin"] {
                use mesh_skin();
            }
            for i in 1..100 {
                ["MeshSkin" i] {
                    use mesh_skin();
                }
            }
            ["MeshSkin"] {
                // Section headers commented out as properties: 'MeshSkin*'[MeshSkin].
                stale {
                    key
                        "[MeshSkin]",
                        "[MeshSkin1]";
                }
            }
            ["HealthBar"] {
                string
                    "AttachToBone",
                    "UnitBarKey";
                int
                    "HealthbarTicks",
                    "ShowTicks",
                    "ShowWhileUntargetable";
                int
                    "HPPerTick",
                    "ParallaxOffset";
                float
                    "Scale",
                    "XOffset",
                    "YOffset";
                vec3 "WorldOffset";
            }
            // The client reads Animation1 to Animation<NumberOfAnimations>.
            ["DefaultAnimations"] {
                let count = int "NumberOfAnimations";
                float "Significance";
                for i in 1..100 {
                    stale unless (count.is_some_and(|count| i <= count)) {
                        string ("Animation" i);
                    }
                }
            }
            // The client reads Particle1 to Particle<NumberOfParticles>, and logs a missing one.
            ["IdleParticles"] {
                let count = int "NumberOfParticles";
                string
                    "BeamParticle",
                    "ChampTargetingParticle",
                    "TowerTargetingParticle",
                    "TowerTargetingParticle2";
                int "BeamShouldAlwayStargetEnemy";
                for i in 1..100 {
                    stale unless (count.is_some_and(|count| i <= count)) {
                        string ("Particle" i);
                    }
                }
                // Keys of MeshSkin pasted into IdleParticles: 16 and 21 files of the corpus.
                stale {
                    float
                        "GameplayCollisionRadius",
                        "SelfIllumination";
                }
            }
            ["Evolution"] {
                string "EvolveTitle";
                int "EnabledWhileDead";
                for i in 1..5 {
                    string
                        ("Spell" i "EvolveDesc"),
                        ("Spell" i "EvolveIcon");
                }
            }
            for sec in ["Info", "Info1"] {
                [sec] {
                    string
                        "IconCircle",
                        "IconSquare";
                    float "IconCircleScale";
                }
            }
            for sec in ["RecItems", "TutorialRecItems"] {
                [sec] {
                    for i in 1..7 {
                        int ("RecItem" i);
                    }
                }
            }
            ["Sounds"] {
                string
                    "Death",
                    "Ready";
                for k in ["Attack", "Click", "Move"] {
                    for i in 1..5 {
                        string (k i);
                    }
                }
                for i in 1..3 {
                    string ("Special" i);
                }
            }
            ["Useable"] {
                string
                    "HeroUseSpell",
                    "MinionUseSpell";
                int
                    "AllowToUseInZombieState",
                    "AllyCanUse",
                    "EnemyCanUse",
                    "GoldRedirectTargetUseableOnly",
                    "IsUseable",
                    "MinionUseable";
                int "CooldownSpellSlot";
            }
            ["ContextualAction"] {
                string "RuleConfigFile";
                // A key of MeshSkin pasted into ContextualAction: 2 skin files of the corpus.
                stale {
                    int "UsesSkinVO";
                }
            }
            ["Inherits"] {
                string "Recommended";
            }
            ["Education"] {
                int
                    "FirstItem",
                    "SkillOrder";
                string "Metadata";
            }
            for i in 1.. while ["AlternateForm" i] string "Switch" {
                string
                    "Name",
                    "Champion";
                for j in 1..5 {
                    string ("Spell" j);
                }
            }
            ["Interaction"] {
                string
                    "DoubleSided",
                    "IdleAnim",
                    "RandomizeIdleAnimPhase";
            }
            ["Minimap"] {
                string "MinimapIconOverride";
            }
            ["Minion"] {
                int
                    "AlwaysUpdatePAR",
                    "AlwaysVisible",
                    "IsTower";
            }
            ["Package"] {
                string
                    "FallbackINI",
                    "FallbackPackage";
            }
        );
    }
}

// DATA/Characters/*/evo.inibin
// DATA/Characters/*/shopevo.inibin
mod evo {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Evo"] {
                string "EvoShopOpen";
            }
        );
    }
}

// DATA/**/*.dds.inibin
// LEVELS/*/MaterialSettings*.inibin
mod material {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Color"] {
                vec3
                    "AmbientColor",
                    "emissiveFactor",
                    "RimColor";
            }
            ["Float"] {
                float
                    "Detail",
                    "EmissiveFactor",
                    "Reflectivity",
                    "RimPower",
                    "SELF_ILLUMINATION",
                    "ShadowIntensity",
                    "Shininess",
                    "SpecGloss",
                    "SpecPowerBase",
                    "SpecMul",
                    "RimMul",
                    "AmbientMul",
                    "AOMul",
                    "SpecGlossTop",
                    "SpecGlossBottom",
                    "ShadowIntensityCharacter";
            }
            ["Shader"] {
                string
                    "Vertex",
                    "Pixel";
                int "HasTangent";
            }
            ["System"] {
                int "Enabled";
            }
            ["Textures"] {
                string
                    "DiffuseMap",
                    "EmissiveMap",
                    "EnvironmentMap",
                    "NormalMap",
                    "SpecMap";
            }
        );
    }
}

// DATA/Characters/*/RecItems*.inibin
// DATA/Characters/*/**/RecItems*.inibin
mod recitems {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for i in 1.. while ["ItemSet" i] int "RecItem1" {
                string "SetName";
                for n in 2..7 {
                    int ("RecItem" n);
                }
            }
        );
    }
}
