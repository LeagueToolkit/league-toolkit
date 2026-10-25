//! The recipe of the scene configuration and the map objects of a level.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    level_mapobjects::read(r);
    level_scene_cfg::read(r);
}

// LEVELS/*/Scene/MapObjects.inibin
mod level_mapobjects {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn object(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            let ty = string "Type";
            int "skinID";
            for k in ["Position", "Scale", "PitchYawRoll"] {
                vec3 k;
            }
            if (ty.is_some_and(|ty| ty.eq_ignore_ascii_case("LevelSize"))) {
                vec3 "LevelSize";
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Header"] {
                int "Version";
                let count = int "ObjectCount";
                // numbered up to ObjectCount, with gaps
                for i in (1..=count.unwrap_or(0)) {
                    if let name = string ("Object" i) {
                        [name] {
                            use object();
                        }
                    }
                }
            }
            stale {
                for n in ["LevelProp_Teemo", "LevelProp_Tryndamere"] {
                    [n] {
                        use object();
                    }
                }
            }
        );
    }
}

// LEVELS/*/Scene/CFG/*.cfgbin
mod level_scene_cfg {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    fn prop(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            vec3
                "Move",
                "Rot";
            int "skinID";
        );
    }

    fn object(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            use prop();
            for i in 1..3 {
                string ("SkinName" i);
                int ("SkinID" i);
            }
            string "Team";
            float
                "SelectionHeight",
                "SelectionRadius",
                "PathfindingCollisionRadius",
                "PerceptionBubbleRadius",
                "PerceptionBubbleRadiusTutorial",
                "Collision Height",
                "Collision Radius";
            string "ArmorMaterial";
            float
                "mMaxHP",
                "mMaxHPTutorial",
                "mMaxMP",
                "mArmor",
                "mBaseStaticHPRegen";
            string "DestroyedTowerSkin";
            int "DestroyedTowerSkinID";
            float
                "SwapModelOnDeathTime",
                "WarningDecalSizeFactor";
            vec3
                "WarningDecalOffset",
                "ShoppingCenter";
            int
                "ShoppingRangeClient",
                "ShoppingRangeServer";
            string "VOSkinName";
            stale {
                float "Selection_T1Radius";
            }
            stale {
                string "";
            }
        );
    }

    fn objects(r: &mut Reader, names: &[&str]) {
        recipe!(r =>
            for n in names {
                [n] {
                    use object();
                }
            }
        );
    }

    fn numbered(r: &mut Reader, base: Parts<'_>, lo: i64, hi: i64) {
        recipe!(r =>
            [base] {
                use object();
            }
            for i in (lo..hi) {
                [base i] {
                    use object();
                }
            }
        );
    }

    fn spawns(r: &mut Reader, prefix: Parts<'_>, suffix: Parts<'_>) {
        recipe!(r =>
            for side in ["Chaos", "Order"] {
                for l in ["C", "L", "R"] {
                    [prefix "__P_" side "_Spawn_Barracks__" l "01" suffix] {
                        use object();
                    }
                }
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["GameObject"] {
                use object();
            }
            for l in ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"] {
                for m in ["sru", "ha"] {
                    let b = name ("LevelProp_brush_" m "_" l);
                    [b] {
                        use prop();
                    }
                    for i in (1..if m.eq_ignore_ascii_case("sru") { 300 } else { 32 }) {
                        [b i] {
                            use prop();
                        }
                    }
                }
            }
            for n in ["bw_a15", "bw_f", "bw_f14", "cs_b18", "cs_d12", "cs_f158", "cs_j2"] {
                ["LevelProp_brush_" n] {
                    use prop();
                }
            }
            for t in ["T1", "T2"] {
                for l in ["C", "L", "R"] {
                    for i in 1..12 {
                        ["Turret_" t "_" l "_0" i] {
                            use object();
                        }
                    }
                }
            }
            use numbered(name!("Turret_ChaosTurretShrine"), 1, 2);
            use numbered(name!("Turret_OrderTurretShrine"), 1, 2);
            for t in ["T1", "T2"] {
                for l in ["C", "L", "R"] {
                    ["Barracks_" t "_" l "1"] {
                        use object();
                    }
                }
            }
            for i in 1..5 {
                ["HQ_T" i] {
                    use object();
                }
            }
            for side in ["Chaos", "Order", "Neutral"] {
                use numbered(name!((side "Shop")), 1, 3);
                for i in 1..8 {
                    [side "Shop0" i] {
                        use object();
                    }
                }
            }
            for p in [
                "", "__", "pasted__", "navfromlive:", "navfromlive1:", "navfromlive2:",
                "pathfrmlive:", "pathfrmlive1:",
            ] {
                use spawns(name!(p), name!(""));
            }
            for m in [1, 2, 3, 4, 6] {
                let dir = name ("Levels\\Map" m "\\SCENE\\");
                for ext in [".sco", ".scb"] {
                    use spawns(name!(dir), name!(ext));
                    use spawns(name!((dir "__")), name!(ext));
                    for n in [
                        "HQ_T1", "HQ_T2", "ChaosShop01", "OrderShop01", "pasted__orderShop01",
                    ] {
                        [dir n ext] {
                            use object();
                        }
                    }
                }
            }
            use numbered(name!("__ANIMS_shopekeeper"), 1, 6);
            use numbered(name!("__ANIMS_store"), 1, 6);
            use numbered(name!("Info_HealthPack0"), 1, 6);
            use numbered(name!("LevelProp_BW_boat"), 1, 6);
            use numbered(name!("LevelProp_bw_bottle"), 1, 7);
            use numbered(name!("LevelProp_BW_dblrope"), 1, 6);
            use numbered(name!("LevelProp_BW_seagull"), 1, 6);
            use numbered(name!("LevelProp_bw_shadowshark"), 1, 6);
            use numbered(name!("LevelProp_BW_shortrope"), 1, 6);
            use numbered(name!("LevelProp_BW_vship"), 1, 4);
            use numbered(name!("LevelProp_FJ_AP_BannerChaos"), 1, 2);
            use numbered(name!("LevelProp_FJ_AP_BannerOrder"), 1, 2);
            use numbered(name!("LevelProp_FJ_AP_Order_Lane_Tower"), 1, 3);
            use numbered(name!("LevelProp_HA_AP_BridgeLaneStatue"), 1, 9);
            use numbered(name!("LevelProp_HA_AP_Chains"), 1, 7);
            use numbered(name!("LevelProp_HA_AP_Chains_Long"), 1, 4);
            use numbered(name!("LevelProp_HA_AP_Cutaway"), 1, 2);
            use numbered(name!("LevelProp_HA_AP_OrderCloth"), 1, 4);
            use numbered(name!("LevelProp_HA_AP_Poro"), 1, 40);
            use numbered(name!("LevelProp_HABW_Lantern"), 1, 6);
            use numbered(name!("LevelProp_KINGPORO_PoroFollower"), 1, 3);
            use numbered(name!("LevelProp_OdinCrane"), 1, 2);
            use numbered(name!("LevelProp_OdinRockSaw0"), 1, 3);
            use numbered(name!("LevelProp_ShopMale"), 0, 3);
            use numbered(name!("LevelProp_SR2_DragonChest"), 1, 6);
            for k in ["platform", "wall"] {
                for side in ["Chaos", "Order"] {
                    use numbered(name!(("LevelProp_Sru_Es_Banner" k "_" side)), 1, 8);
                }
            }
            use numbered(name!("LevelProp_SRU_SnowDown_Gingerbread"), 1, 13);
            use numbered(name!("LevelProp_Sruap_Pali_Statue_Banner"), 1, 2);
            use numbered(name!("LevelProp_SwainBeam"), 1, 4);
            use numbered(name!("LevelProp_TT_Brazier"), 1, 9);
            use numbered(name!("LevelProp_TT_Nexus_Gears"), 1, 2);
            use numbered(name!("LevelProp_TT_Shopkeeper"), 1, 2);
            use numbered(name!("LevelProp_Yonkey"), 1, 2);
            use objects(&[
                "__ANIMS_chaosShrineTurret", "__ANIMS_orderShrineTurret", "__Spawn_T1",
                "__Spawn_T2", "ChaosTorch05", "Info_GraveyardA", "Info_NeutralA",
                "Info_NeutralA_sub1", "Info_NeutralA_sub2", "Info_NeutralC", "Info_PointA",
                "LevelProp_BW_AP_Bubbs", "LevelProp_BW_AP_Finn", "LevelProp_BW_brdgdoor1",
                "LevelProp_bw_Hcannon", "LevelProp_BW_shrkhk1", "LevelProp_BW_signc1",
                "LevelProp_BW_squid1", "LevelProp_BW_statlant1",
                "LevelProp_FJ_AP_Banner_Mid_Bridge", "LevelProp_FJ_AP_Bridge_Lane_Statue",
                "LevelProp_FJ_AP_Chains", "LevelProp_FJ_AP_Order_CutAway",
                "LevelProp_FJ_AP_Order_Hero_Tower", "LevelProp_FJ_AP_Periph_Bridge",
                "LevelProp_FJ_AP_ShopSouth", "LevelProp_HA_AP_BannerMidBridge",
                "LevelProp_HA_AP_Hermit", "LevelProp_HA_AP_Hermit_Robot1",
                "LevelProp_HA_AP_HeroTower", "LevelProp_HA_AP_PeriphBridge",
                "LevelProp_HA_AP_ShpNorth", "LevelProp_HA_AP_ShpSouth", "LevelProp_HA_AP_Viking",
                "LevelProp_Odin_Drill", "LevelProp_Odin_Lifts_Buckets",
                "LevelProp_Odin_Lifts_Crystal", "LevelProp_Odin_skeleton",
                "LevelProp_Odin_SoG_Chaos", "LevelProp_Odin_SOG_Chaos_Crystal",
                "LevelProp_Odin_SoG_Order", "LevelProp_Odin_SOG_Order_Crystal",
                "LevelProp_NPC_WorldsCup", "LevelProp_Odin_Windmill_Gears",
                "LevelProp_Odin_Windmill_Propellers", "LevelProp_OdinClaw", "LevelProp_Shop_Hermit",
                "LevelProp_ShopFemale", "LevelProp_Shopkeeper_Male", "LevelProp_SRU_PoroSprite1",
                "LevelProp_SRU_ShopBeast", "LevelProp_SRU_storeBeast",
                "LevelProp_SRU_storeKeeperNorth", "LevelProp_SRU_storekeepersouth",
                "LevelProp_Teemo", "LevelProp_Tryndamere", "LevelProp_TT_Chains_Bot_Lane",
                "LevelProp_TT_Chains_Order_Base", "LevelProp_TT_Chains_Order_Periph",
                "LevelProp_TT_Chains_Xaos_Base", "Prop_SouthShop_Chest", "Shopkeeper_North",
                "SRU_ShopBeast", "Test_SouthShopkeeper",
            ]);
            ["General"] {
                vec3
                    "DD",
                    "FarTint",
                    "RefractTint",
                    "UnderwaterTint";
                float
                    "Distort",
                    "WaterDepth",
                    "WaterY";
                int
                    "Fog",
                    "TileFrequency";
                for i in 1..3 {
                    int ("Fresnel" i "Pow");
                    float
                        ("Fresnel" i "Val"),
                        ("ScrollSpeed" i);
                    int
                        ("ScrollX" i),
                        ("ScrollY" i);
                }
            }
        );
    }
}
