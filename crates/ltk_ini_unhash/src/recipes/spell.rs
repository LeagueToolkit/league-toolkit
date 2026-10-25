//! The recipe of spell files: spells, buffs, talents, items and item groups.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    buff::read(r);
    item::read(r);
    itemgroup::read(r);
    spell_data::read(r);
    talent::read(r);
}

// DATA/Buffs/*.inibin
mod buff {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["BuffData"] {
                string "DynamicTooltip";
                for name in ["FloatStaticsDecimals", "FloatVarsDecimals"] {
                    int name;
                    for i in 1..10 {
                        int (name i);
                    }
                }
            }
        );
    }
}

// DATA/Items/*.inibin
// DATA/Characters/*/Items/**/*.inibin
mod item {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    const BOTH: &[&str] = &[
        "Armor",
        "AttackSpeed",
        "Block",
        "CritChance",
        "CritDamage",
        "Dodge",
        "HPPool",
        "HPRegen",
        "MagicDamage",
        "MagicPenetration",
        "MovementSpeed",
        "MPPool",
        "MPRegen",
        "PhysicalDamage",
        "SpellBlock",
    ];

    fn mods(r: &mut Reader, s: Sec, prefix: Parts<'_>, stats: &[&str], suffixes: &[&str]) {
        recipe!(r, s =>
            for st in stats {
                for x in suffixes {
                    float (prefix st x);
                }
            }
        );
    }

    fn build_list(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            for i in 1..21 {
                int ("Item" i);
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Data"] {
                string
                    "Description",
                    "DisplayName",
                    "DynamicTooltip",
                    "ShortDescription",
                    "DisabledDescriptionOverride",
                    "InventoryIcon",
                    "ImagePath",
                    "ItemGroup",
                    "RequiredChampion",
                    "RequiredSpellName",
                    "SpellName";
                int
                    "CanBeDropped",
                    "CanBeSold",
                    "ClearUndoHistoryOnActivate",
                    "Clickable",
                    "Consumed",
                    "DisappersOnDeath",
                    "DropsOnDeath",
                    "ForceLoad",
                    "HideFromAll",
                    "InStore",
                    "IsRecipe",
                    "ItemCalloutPlayer",
                    "ItemCalloutSpectator",
                    "ShowInActiveItemDisplay",
                    "UsableInStore",
                    "UseEffect",
                    "UseWhenAcquired";
                int
                    "BuildDepth",
                    "CategoryId",
                    "EffectRadius",
                    "Epicness",
                    "ItemClass",
                    "ItemId",
                    "ItemType",
                    "MaxStack",
                    "Price",
                    "RequiredLevel",
                    "SpecialRecipe";
                float
                    "CooldownShowDisabledDuration",
                    "SellBackModifier",
                    "SidegradeCredit";
                for i in 1..5 {
                    int ("RecipeItem" i);
                }
                seq int "RequiredItem" from 1;
                seq int "Sidegrade" from 1;
                for i in 1..25 {
                    float ("Effect" i "Amount");
                }
            }
            ["Data"] {
                use mods(name!("Flat"), BOTH, &["Mod"]);
                use mods(name!("Percent"), BOTH, &["Mod"]);
                use mods(name!("Flat"), &["EnergyPool", "EnergyRegen"], &["Mod"]);
                use mods(name!("Percent"), &[
                    "BaseHPRegen", "BaseMPRegen", "Cooldown", "Hardness", "LifeSteal", "SlowResist",
                    "SpellVamp", "TenacityItem",
                ], &["Mod"]);
                float
                    "FlatEXPBonus",
                    "PercentEXPBonus";
                use mods(name!("rFlat"), &[
                    "ArmorPenetration", "Dodge", "MagicPenetration", "TimeDead",
                ], &["Mod", "ModPerLevel"]);
                use mods(name!("rFlat"), &[
                    "Armor", "CritChance", "CritDamage", "Energy", "EnergyRegen", "HP", "HPRegen",
                    "MagicDamage", "MovementSpeed", "MP", "MPRegen", "PhysicalDamage", "SpellBlock",
                ], &["ModPerLevel"]);
                use mods(name!("rPercent"), &[
                    "ArmorPenetration", "Cooldown", "MagicPenetration", "TimeDead",
                ], &["Mod", "ModPerLevel"]);
                use mods(name!("rPercent"), &["AttackSpeed", "MovementSpeed"], &["ModPerLevel"]);
                float
                    "rFlatGoldPer10Mod",
                    "rPhysicalLethality";
                string
                    "AvatarItemType",
                    "AvatarUniqueEffect";
                int
                    "AvatarDuration",
                    "AvatarLevelRequired",
                    "PlatformEnabled",
                    "SummonerItem";
                int "AvatarTierRequired";
            }
            ["Builds"] {
                use build_list();
            }
            ["Categories"] {
                int
                    "Active",
                    "Armor",
                    "ArmorPenetration",
                    "AttackSpeed",
                    "Aura",
                    "Boots",
                    "Consumable",
                    "CooldownReduction",
                    "CriticalStrike",
                    "Damage",
                    "GoldPer",
                    "Health",
                    "HealthRegen",
                    "Jungle",
                    "Lane",
                    "LifeSteal",
                    "MagicPenetration",
                    "Mana",
                    "ManaRegen",
                    "Movement",
                    "NonbootsMovement",
                    "OnHit",
                    "Permanent",
                    "Reduction",
                    "Slow",
                    "SpellBlock",
                    "SpellDamage",
                    "SpellVamp",
                    "Stealth",
                    "Tenacity",
                    "Trinket",
                    "Vision";
                stale {
                    use build_list();
                }
            }
        );
    }
}

// DATA/Items/ItemGroups/*.inibin
mod itemgroup {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Data"] {
                for n in [
                    "InventorySlotMax", "InventorySlotMin", "MaxGroupOwnable", "PurchaseCooldown",
                ] {
                    int n;
                }
            }
        );
    }
}

// DATA/Spells/*.inibin
// DATA/Shared/Spells/*.inibin
// DATA/Items/Spells/*.inibin
// DATA/Characters/*/Spells/**/*.inibin
// DATA/Characters/*/Scripts/*.inibin
// DATA/Characters/*/*.inibin
// not DATA/Characters/*/Scripts/*BotSummonerSpells*.inibin
// not DATA/Characters/*/Scripts/*BotRunes.inibin
// not DATA/Characters/*/Scripts/*BotMasteries.inibin
// not DATA/Characters/<c>/<c>.inibin
// not DATA/Characters/*/RecItems*.inibin
// not DATA/Characters/<c>_OVERRIDE/<c>.inibin
// not DATA/Characters/<c>/Spells/<c>.inibin
// not DATA/Characters/Sion/SionBerserk.inibin
// not DATA/Characters/Sion/SionMax.inibin
// not DATA/Characters/Sion/SionPassive.inibin
// not DATA/**/*.dds.inibin
// not DATA/Characters/*/evo.inibin
// not DATA/Characters/*/shopevo.inibin
mod spell_data {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};
    use crate::Ty;

    fn levels(r: &mut Reader, s: Sec, name: Parts<'_>, ty: Option<Ty>) {
        recipe!(r, s =>
            read (ty) name;
            for i in 0..7 {
                read (ty) (name i);
            }
        );
    }

    fn effects(r: &mut Reader, s: Sec, prefix: Parts<'_>, n: i64) {
        recipe!(r, s =>
            for e in (1..n + 1) {
                for l in 0..7 {
                    float (prefix "Effect" e "Level" l "Amount");
                }
            }
        );
    }

    fn locator(r: &mut Reader, s: Sec, loc: Parts<'_>) {
        recipe!(r, s =>
            string
                loc,
                (loc "_BasePosition"),
                (loc "_OrientationType");
            float
                (loc "_DistanceOffset"),
                (loc "_AngleOffset");
        );
    }

    fn targeter(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "DrawableType",
                "FallbackDirection",
                "CenterPosition",
                "RangeIndicatorTextureName",
                "TextureBaseMaxGrow",
                "TextureBaseOverride",
                "TextureCone",
                "TextureMaxGrow",
                "TextureOrientation",
                "TextureOverride",
                "TextureTargetMaxGrow",
                "TextureTargetOverride",
                "TextureWall",
                "OverrideTexture";
            int
                "AlwaysDraw",
                "Facing",
                "FacingLine",
                "HasMaxGrowRangeTexture",
                "HideWithLineIndicator",
                "IsConstrainedToRange",
                "LineStopsAtEndPosition",
                "UseGlobalLineIndicator",
                "UseMinimapTargeting";
            float
                "ArrowSize",
                "ConeAngle",
                "ConeRange",
                "ConstraintRange",
                "MaxAngle",
                "MaxAngleRangeFactor",
                "MinAngle",
                "MinAngleRangeFactor",
                "MinimumDisplayedRange",
                "OverrideRadius",
                "Range",
                "Thickness";
            for k in [
                "LineWidth", "OverrideBaseRange", "Length", "RangeGrowthDuration", "RangeGrowthMax",
            ] {
                use levels(name!(k), Some(Ty::Float));
            }
            for loc in ["Start", "End", "Center"] {
                use locator(name!(loc));
            }
            string
                "ConstraintPos",
                "ConstraintPos_DistanceOffset";
            stale {
                string "AlternateName";
                for i in 1..6 {
                    float ("CastRange" i);
                }
                int
                    "LineMissileCollisionFromStartPoint",
                    "LineMissileEndsAtTargetPoint",
                    "LineMissileTrackUnits",
                    "MissilePerceptionBubbleRevealsStealth",
                    "OverrideForceSpellAnimation",
                    "OverrideForceSpellCancel",
                    "SpellRevealsChampion";
                float
                    "LineMissileDelayDestroyAtEndSeconds",
                    "LineMissileTimePulseBetweenCollisionSpellHits",
                    "MissilePerceptionBubbleRadius";
            }
        );
    }

    fn buff_data(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "AlternateName",
                "Description",
                "DisplayName",
                "DynamicExtended",
                "DynamicTooltip",
                "InventoryIcon",
                "Sound_VOEventCategory";
            int
                "ApplyMaterialOnHitSound",
                "HideDurationInUI",
                "ShowInActiveItemDisplay",
                "ShowInTrackerUI";
            int
                "FloatStaticsDecimals",
                "FloatVarsDecimals";
            for i in 1..10 {
                int
                    ("FloatStaticsDecimals" i),
                    ("FloatVarsDecimals" i);
            }
        );
    }

    fn unit(r: &mut Reader, s: Sec, ms: Sec) {
        recipe!(r, s =>
            int
                "AlwaysVisible",
                "IsMelee",
                "NeverRender",
                "NoAutoAttack",
                "NoHealthBar",
                "ShouldFaceTarget";
            float
                "AcquisitionRange",
                "Armor",
                "ArmorPerLevel",
                "AttackDelayCastOffsetPercent",
                "AttackDelayOffsetPercent",
                "AttackRange",
                "AttackSpeed",
                "AttackSpeedPerLevel",
                "BaseCritChance",
                "BaseDamage",
                "BaseDodge",
                "BaseFactorHPRegen",
                "BaseFactorMPRegen",
                "BaseHP",
                "BaseMP",
                "BaseSpellEffectiveness",
                "BaseStaticHPRegen",
                "BaseStaticMPRegen",
                "CritDamageBonus",
                "CritPerLevel",
                "DamagePerLevel",
                "ExpGivenOnDeath",
                "GameplayCollisionRadius",
                "GoldGivenOnDeath",
                "HPPerLevel",
                "HPRegenPerLevel",
                "LevelDodge",
                "LevelSpellEffectiveness",
                "MoveSpeed",
                "MPPerLevel",
                "MPRegenPerLevel",
                "PathfindingCollisionRadius",
                "PerceptionBubbleRadius",
                "SelectionHeight",
                "SelectionRadius",
                "SoulGivenOnDeath",
                "SpellBlock",
                "SpellBlockPerLevel",
                "BaseAttackSpeedBonus";
            string
                "ArmorMaterial",
                "Description",
                "EnemyTooltip",
                "FriendlyTooltip",
                "Name",
                "WeaponMaterial";
            for i in 1..5 {
                string ("Spell" i);
            }
            with ms {
                string
                    "Animations",
                    "Body",
                    "ChampionSkinName",
                    "SimpleSkin",
                    "Skeleton",
                    "Texture",
                    "Weight";
                float
                    "SelfIllumination",
                    "SkinScale";
            }
            ["IdleParticles"] {
                int "NumberOfParticles";
                for i in 1..9 {
                    string ("Particle" i);
                }
            }
            ["Info"] {
                string
                    "IconCircle",
                    "IconSquare";
            }
        );
    }

    // A file in a spell folder is a unit if it names the unit or its mesh.
    fn units(r: &mut Reader) {
        let data = r.section(name!("Data"));
        let mesh_skin = r.section(name!("MeshSkin"));
        if r.has(data, name!("Name")) || r.has(mesh_skin, name!("SimpleSkin")) {
            unit(r, data, mesh_skin);
        }
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["SpellData"] {
                int
                    "AIEndOnly",
                    "AISendEvent",
                    "AllowWhileTaunted",
                    "AlwaysSnapFacing",
                    "AmmoCountHiddenInUI",
                    "AmmoNotAffectedByCDR",
                    "ApplyAttackDamage",
                    "ApplyAttackEffect",
                    "ApplyMaterialOnHitSound",
                    "BelongsToAvatar",
                    "CanCastOrQueueWhileCasting",
                    "CanCastWhileDisabled",
                    "CanMoveWhileChanneling",
                    "CannotBeSuppressed",
                    "CanOnlyCastWhileDead",
                    "CanOnlyCastWhileDisabled",
                    "CantCancelWhileChanneling",
                    "CantCancelWhileWindingUp",
                    "CantCastWhileRooted",
                    "CastRangeUseBoundingBoxes",
                    "CastRangeUseMapScaling",
                    "ConsideredAsAutoAttack",
                    "CostAlwaysShownInUi",
                    "CursorChangesInGrass",
                    "CursorChangesInTerrain",
                    "DeathRecap",
                    "DisableCastBar",
                    "DisableChannelBar",
                    "DoesntBreakChannels",
                    "DoNotNeedToFaceTarget",
                    "DrawSecondaryLineIndicator",
                    "HaveAfterEffect",
                    "HaveHitBone",
                    "HaveHitEffect",
                    "HavePointEffect",
                    "HideRangeIndicatorWhenCasting",
                    "IgnoreAnimContinueUntilCastFrame",
                    "IgnoreRangeCheck",
                    "IsDisabledWhileDead",
                    "IsToggleSpell",
                    "LeftClickSpellAction",
                    "LineMissileBounces",
                    "LineMissileCollisionFromStartPoint",
                    "LineMissileEndsAtTargetPoint",
                    "LineMissileFollowsTerrainHeight",
                    "LineMissileTrackUnits",
                    "LineMissileTrackUnitsAndContinues",
                    "LineMissileUsesAccelerationForBounce",
                    "LockConeToPlayer",
                    "MinimapIconDisplayFlag",
                    "MinimapIconRotation",
                    "MissileBlockTriggersOnDestroy",
                    "MissileFollowsTerrainHeight",
                    "MissilePerceptionBubbleRevealsStealth",
                    "MissileUnblockable",
                    "NoWinddownIfCancelled",
                    "OrientRadiusTextureFromPlayer",
                    "OrientRangeIndicatorToCursor",
                    "OrientRangeIndicatorToFacing",
                    "OverrideForceSpellAnimation",
                    "OverrideForceSpellCancel",
                    "PlatformEnabled",
                    "PreventChargingSecondCast",
                    "RightClickSpellAction",
                    "ShowChannelBar",
                    "ShowInTrackerUI",
                    "SpellRevealsChampion",
                    "SubjectToGlobalCooldown",
                    "TriggersGlobalCooldown",
                    "UpdateRotationWhenCasting",
                    "UseAnimatorFramerate",
                    "UseAutoattackCastTime",
                    "UseChargeChanneling",
                    "UseChargeTargeting",
                    "UseGlobalLineIndicator",
                    "UseMinimapTargeting";
                float
                    "AILifetime",
                    "AIRadius",
                    "AIRange",
                    "AISpeed",
                    "ArcHeight",
                    "AttackDelayCastOffsetPercent",
                    "BounceRadius",
                    "CancelChargeOnRecastTime",
                    "CastConeAngle",
                    "CastConeDistance",
                    "CastFrame",
                    "CastTargetAdditionalUnitsRadius",
                    "ChargeUpdateInterval",
                    "CircleMissileAngularVelocity",
                    "CircleMissileRadialVelocity",
                    "DeathRecapPriority",
                    "DelayCastOffsetPercent",
                    "DelayTotalTimePercent",
                    "HeatCost",
                    "LineDragLength",
                    "LineMissileDelayDestroyAtEndSeconds",
                    "LineMissileTargetHeightAugment",
                    "LineMissileTimePulseBetweenCollisionSpellHits",
                    "LineMissileWidthPerSecond",
                    "LineWidth",
                    "LuaOnMissileUpdateDistanceInterval",
                    "MissileAccel",
                    "MissileFixedTravelTime",
                    "MissileGravity",
                    "MissileLifetime",
                    "MissileMaxSpeed",
                    "MissileMinSpeed",
                    "MissileMinTravelTime",
                    "MissilePerceptionBubbleRadius",
                    "MissileSpeed",
                    "MissileTargetHeightAugment",
                    "OverrideCastTime",
                    "SpellCastTime",
                    "SpellTotalTime",
                    "StartCooldown",
                    "TargeterConstrainedToRange",
                    "Coefficient",
                    "Coefficient2",
                    "Coefficient3";
                int
                    "AffectsStatusFlags",
                    "AffectsTypeFlags",
                    "AIBlockLevel",
                    "AvatarLevelRequired",
                    "AvatarMinLevel",
                    "CastType",
                    "Flags",
                    "HitEffectOrientType",
                    "NumSpellTargeters",
                    "SpellId",
                    "TargettingType",
                    "Version";
                string
                    "AffectsStatusTextFlags",
                    "AffectsTypeTextFlags",
                    "AfterEffectName",
                    "AlternateName",
                    "AnimationLeadOutName",
                    "AnimationLoopName",
                    "AnimationName",
                    "AnimationWinddownName",
                    "CastRadiusSecondaryTexture",
                    "CastRadiusTexture",
                    "CastRangeSecondaryTexture",
                    "CastRangeTexture",
                    "CastRangeTextureOverrideName",
                    "ClientOnlyMissileTargetBoneName",
                    "ConeTargetingTextureOverrideName",
                    "Description",
                    "DisplayName",
                    "DynamicExtended",
                    "DynamicTooltip",
                    "ExcludedUnitTags",
                    "GameModes",
                    "HitBoneName",
                    "HitEffectName",
                    "HitEffectPlayer",
                    "HitEffectPlayerName",
                    "KeywordWhenAcquired",
                    "LineTargetingBaseTextureOverrideName",
                    "LineTargetingTargetTextureOverrideName",
                    "LookAtPolicy",
                    "MaxGrowthLineBaseTextureName",
                    "MaxGrowthLineTargetTextureName",
                    "MaxGrowthRangeTextureName",
                    "MinimapIcon",
                    "MissileBoneName",
                    "MissileEffect",
                    "MissileEffectPlayer",
                    "PointEffectName",
                    "RangeIndicatorTextureName",
                    "RangeIndicatorTextureOverrideName",
                    "RequiredUnitTags",
                    "SelectionPreference",
                    "Sound_CastName",
                    "Sound_HitName",
                    "Sound_VOEventCategory",
                    "SpellTags",
                    "Tags",
                    "TextFlags";
                vec3 "ParticleStartOffset";
                for i in 1..8 {
                    float ("x" i);
                }
                for k in [
                    "Cooldown", "ManaCost", "AmmoRechargeTime", "ChannelDuration", "CastRange",
                    "CastRangeGrowthMax", "CastRangeGrowthDuration", "CastRadius",
                    "CastRadiusSecondary", "CastRangeDisplayOverride", "CastRangeSecondary",
                    "ManaCostUiOverride", "LocationTargettingLength", "LocationTargettingWidth",
                ] {
                    use levels(name!(k), Some(Ty::Float));
                }
                use levels(name!("MaxAmmo"), Some(Ty::Int));
                use levels(name!("AmmoUsed"), Some(Ty::Int));
                use effects(name!(""), 11);
                for m in ["Map8_", "Map10_"] {
                    for k in ["Cooldown", "CastRange", "CastRangeDisplayOverride"] {
                        use levels(name!((m k)), Some(Ty::Float));
                    }
                    use effects(name!(m), 6);
                }
                int
                    "FloatStaticsDecimals",
                    "FloatVarsDecimals";
                for i in 1..13 {
                    int
                        ("FloatStaticsDecimals" i),
                        ("FloatVarsDecimals" i);
                }
                for i in 1..7 {
                    string ("Level" i "Desc");
                }
                string "InventoryIcon";
                for i in 0..9 {
                    string ("InventoryIcon" i);
                }
                use levels(name!("SecondaryInventoryIcon"), Some(Ty::String));
                stale {
                    int
                        "CantCancelWhileChannelling",
                        "IgnoreAnimContinueUltilCastFrame",
                        "ubjectToGlobalCooldown",
                        "mCanMoveWhileChanneling";
                    int "FloarVarsDecimals1";
                    float
                        "Effect5Level1AAmount",
                        "0MissilePerceptionBubbleRadius";
                }
            }
            for i in 1..11 {
                ["SpellTargeter" i] {
                    if has "DrawableType" {
                        use targeter();
                    }
                    stale {
                        key ("[SpellTargeter" (i + 1) "]");
                    }
                }
            }
            ["OffsetTargeting"] {
                string
                    "OT_ArcTextureOverride",
                    "OT_AreaTextureOverride",
                    "OT_LineBaseTextureOverride",
                    "OT_LineNoIndicatorRadiusTextureOverride",
                    "OT_LineTargetTextureOverride";
                int
                    "OT_DisplaysArcTargeter",
                    "OT_DisplaysAreaIndicator",
                    "OT_DisplaysLineIndicator",
                    "OT_IsArcDirectionLeft",
                    "OT_LineEndsAtTargetPoint";
                float
                    "OT_ArcThicknessOffset",
                    "OT_LineWidth";
                use levels(name!("OT_AreaRadius"), Some(Ty::Float));
                use levels(name!("OT_LineLength"), Some(Ty::Float));
            }
            ["SecondaryTargeting"] {
                string
                    "LineTargetingBaseTextureOverrideName",
                    "LineTargetingTargetTextureOverrideName";
                float "LineWidth";
                int "TargettingType";
            }
            ["SpawningUI"] {
                string "BuffNameFilter";
                int "MaxNumberOfUnits";
            }
            ["BuffData"] {
                use buff_data();
                stale {
                    for l in 1..6 {
                        float ("Effect1Level" l "Amount");
                    }
                    float
                        "LocationTargettingLength6",
                        "LocationTargettingWidth6",
                        "MissileFixedTravelTime",
                        "MissileTargetHeightAugment";
                }
            }
            ["BuffDataData"] {
                stale {
                    use buff_data();
                }
            }
            use units();
            ["Data"] {
                float
                    "SpellDelayCastTime",
                    "SpellDelayTotalTime";
            }
        );
    }
}

// DATA/Talents/*.inibin
mod talent {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["SpellData"] {
                string "Name";
                int "Ranks";
            }
        );
    }
}
