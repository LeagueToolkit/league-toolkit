//! The recipe of particle settings and `.prtbin` particles.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    particles_default::read(r);
    prtbin::read(r);
}

// DATA/Particles/*.inibin
mod particles_default {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["GlobalEffects"] {
                string
                    "Default",
                    "MinionDeath",
                    "FriendlyMinionDeath",
                    "EnemyMinionDeath",
                    "TurretDeath",
                    "FriendlyTurretDeath",
                    "EnemyTurretDeath",
                    "ChaosBuildingDeath",
                    "OrderBuildingDeath",
                    "ChaosBarrackDeath",
                    "OrderBarrackDeath",
                    "ChaosBarracksSpawn",
                    "OrderBarracksSpawn",
                    "ChaosBarracksPoweredUp",
                    "OrderBarracksPoweredUp",
                    "ChaosBarracksDamperRespawn",
                    "OrderBarracksDamperRespawn",
                    "ChaosHQDeath",
                    "OrderHQDeath",
                    "ChaosHQDeath2",
                    "OrderHQDeath2",
                    "ChaosSpawnTurretBeam",
                    "OrderSpawnTurretBeam",
                    "LaserSightBeam",
                    "OrderLaserSightBeam",
                    "LevelUp",
                    "HeroKilledByNeutral",
                    "HeroKilledByHero",
                    "HeroKilledByPlayer",
                    "HeroKilledAssistedByPlayer",
                    "HeroEnemyKilled",
                    "HeroAllyKilled",
                    "HeroRespawn",
                    "CursorMoveTo",
                    "FountainHeal",
                    "FocusTarget",
                    "DefendTarget",
                    "CommandTarget",
                    "CommandGround",
                    "ComeHere",
                    "OMW",
                    "Retreat",
                    "Danger",
                    "MIA",
                    "GoldAcquisition",
                    "MissingInstant",
                    "MissingLingering";
                int "MaxIdentifier";
                for i in 0.. while string ("EffectName" i) {
                    string ("FileName" i);
                }
            }
            [".ShellClassInfo"] {
                string "IconFile";
                int "IconIndex";
            }
            ["MeshSkin"] {
                string
                    "SimpleSkin",
                    "Skeleton",
                    "Texture";
            }
            ["SpellData"] {
                int
                    "ApplyAttackDamage",
                    "BelongsToAvatar",
                    "CantCancelWhileChanneling",
                    "CantCancelWhileWindingUp",
                    "HaveAfterEffect",
                    "HaveHitBone",
                    "HaveHitEffect",
                    "HavePointEffect",
                    "IsDisabledWhileDead",
                    "IsToggleSpell",
                    "LineMissileBounces",
                    "SubjectToGlobalCooldown",
                    "TriggersGlobalCooldown";
                int
                    "CastType",
                    "Flags",
                    "TargettingType",
                    "Version";
                float
                    "CastConeAngle",
                    "CastConeDistance",
                    "CastFrame",
                    "CastRadius",
                    "CastRange",
                    "CastTargetAdditionalUnitsRadius",
                    "Cooldown",
                    "DelayCastOffsetPercent",
                    "DelayTotalTimePercent",
                    "LineWidth",
                    "MissileSpeed",
                    "StartCooldown";
                string
                    "AfterEffectName",
                    "AnimationLoopName",
                    "AnimationName",
                    "AnimationWinddownName",
                    "Description",
                    "HitBoneName",
                    "HitEffectName",
                    "InventoryIcon",
                    "MissileBoneName",
                    "MissileEffect",
                    "PointEffectName",
                    "SelectionPreference",
                    "Sound_CastName",
                    "Sound_HitName",
                    "TextFlags";
                seq float "ManaCost" from 1;
                for i in 1..6 {
                    string ("Level" i "Desc");
                }
            }
        );
    }
}

// **/*.prtbin
mod prtbin {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};
    use crate::Ty;

    fn gradient(r: &mut Reader, s: Sec, g: Parts<'_>, ty: Option<Ty>) {
        recipe!(r, s =>
            let n = int (g "Num");
            for i in (1..n.unwrap_or(8) + 1) {
                read (ty) (g i);
            }
        );
    }

    fn element(r: &mut Reader, e: Sec) {
        recipe!(r, e =>
            int
                "StartFrame",
                "NumFrames",
                "IsSingleParticle",
                "PowerTint",
                "DirectionOriented",
                "NumberOfParticles";
            float
                "FrameRate",
                "UVSpeed",
                "StartTime",
                "EndTime",
                "LifeTime",
                "Size",
                "Velocity",
                "Weight",
                "Spin",
                "LifeVariation",
                "SizeVariation",
                "VelocityVariation",
                "WeightVariation",
                "SpinVariation",
                "TrailStepDist",
                "RayWidth";
            vec3
                "MotionRandomness",
                "BornArea",
                "DirectionRandomness",
                "MotionRandomnessVariation",
                "EmmiterOffset";
            let random = int "EmitAtRandomDirection";
            stale unless (random.is_some_and(|random| random != 0)) {
                vec3 "RandomDirectionVector";
            }
            for g in ["ColorOverLife", "AngleXOverLife", "AngleYOverLife", "AngleZOverLife"] {
                use gradient(name!(g), Some(Ty::Vec4));
            }
            for g in [
                "OpacityOverLife", "SizeOverLife", "VelocityOverLife", "WeightOverLife",
                "SpinOverLife", "BindWeightOverLife",
            ] {
                use gradient(name!(g), None);
            }
            let quad_type = int "QuadType";
            stale unless (quad_type == Some(3)) {
                string
                    "MeshFile",
                    "MeshTexture";
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for i in 1..9 {
                ["System"] {
                    if let name = string ("ParticleType" i) {
                        [name] {
                            if (!(name.eq_ignore_ascii_case("-NONE-"))) {
                                use element();
                            }
                        }
                    }
                }
            }
            ["System"] {
                let distortion = int "HaveDistortionTexture";
                stale unless (distortion.is_some_and(|distortion| distortion != 0)) {
                    string "DistortionTexture";
                }
                string "Texture";
                float
                    "XDiv",
                    "YDiv",
                    "GlobalScale",
                    "EmitRadius",
                    "EmitTime",
                    "BuildUpTime";
                int
                    "BlendMode",
                    "MeshDisableBackfaceCull",
                    "ZWriteAlwaysDisabled";
                vec3
                    "Direction",
                    "TintColor";
            }
        );
    }
}
