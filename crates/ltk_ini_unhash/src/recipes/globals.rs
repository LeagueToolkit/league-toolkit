//! The recipe of `DATA/Globals` files, bot loadouts, hero spawn offsets and item categories.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    bot_masteries::read(r);
    bot_runes::read(r);
    bot_summonerspells::read(r);
    globals_abgroups::read(r);
    globals_bounty::read(r);
    globals_colorsettings::read(r);
    globals_critical::read(r);
    globals_gamemutatorexpansions::read(r);
    globals_healthgain::read(r);
    globals_pinglimit::read(r);
    globals_quests::read(r);
    globals_stats::read(r);
    globals_tips::read(r);
    globals_unitstats::read(r);
    herospawnoffsets::read(r);
    item_categories::read(r);
}

// DATA/Characters/*/Scripts/*BotMasteries.inibin
mod bot_masteries {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for d in ["eINTRO", "eTUTORIAL", "eNEWBIE", "eINTERMEDIATE", "eADVANCED", "eUBER"] {
                ["BotMasteryData" d] {
                    for i in 1.. while int ("Mastery" i) {
                        int ("Mastery" i "Level");
                    }
                }
            }
        );
    }
}

// DATA/Characters/*/Scripts/*BotRunes.inibin
mod bot_runes {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for d in ["eINTRO", "eTUTORIAL", "eNEWBIE", "eINTERMEDIATE", "eADVANCED", "eUBER"] {
                ["BotRuneData" d] {
                    seq int "Rune" from 1;
                }
            }
        );
    }
}

// DATA/Characters/*/Scripts/*BotSummonerSpells*.inibin
mod bot_summonerspells {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for d in ["eINTRO", "eTUTORIAL", "eNEWBIE", "eINTERMEDIATE", "eADVANCED", "eUBER"] {
                ["BotSummonerSpellData" d] {
                    string
                        "SummonerSpell1",
                        "SummonerSpell2";
                }
            }
        );
    }
}

// DATA/Globals/ABGroups.inibin
mod globals_abgroups {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn group(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "Features";
            int "Weight";
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Settings"] {
                string "EnabledGroups";
                for p in ["EUN1", "EUNE", "EUW", "EUW1", "PBE1"] {
                    string ("EnabledGroups_" p);
                }
            }
            for g in [
                "Control", "Everything", "NoAggroIndicatorsAndTips", "NoItemShop", "NoQuests",
            ] {
                [g] {
                    use group();
                }
            }
            for i in 1..5 {
                ["EverythingAA" i] {
                    use group();
                }
            }
        );
    }
}

// DATA/Globals/bounty*.inibin
mod globals_bounty {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn bounty(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            int
                "BaseGold",
                "FirstBloodBonus",
                "MinKillsForStreak",
                "MinDeathsForStreak",
                "MinAssistsForStreak",
                "AssistStreakMin",
                "AssistPoolMaxValueTime",
                "AssistPoolMinValueTime",
                "BountyRoundDownIncrement",
                "MinMinionGoldValueForBounty",
                "TimeToMaxValueInSeconds",
                "TimeToMinValueInSeconds";
            float
                "KillStreakBonus",
                "MaxKillStreakBonus",
                "DeathStreakPenalty",
                "MinDeathStreakPenalty",
                "GoldPoolForAssist",
                "AssistDurationOverride",
                "AssistDeathstreakReduction",
                "AssistGoldPerStreak",
                "AssistGoldStreakCap",
                "AssistGoldStreakStart",
                "AssistStreakBonus",
                "AssistPoolMax",
                "AssistPoolMin",
                "MinionGoldValueForBounty",
                "PercentBountyResetOnDeath",
                "TimeBasedMinValuePercent";
            for n in ["One", "Two", "Three", "Four", "Five"] {
                float ("BountyLevel" n "Gold");
            }
            float "MinionGoldDeathstreakReductionRatio";
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for m in [
                "Global", "ARAM", "ASCENSION", "ASSASSINATE", "DARKSTAR", "FIRSTBLOOD", "KingPoro",
                "ODIN", "ONEFORALL", "PROJECT", "SIEGE", "STARGUARDIAN",
            ] {
                [m] {
                    use bounty();
                }
            }
            ["DeathStreak"] {
                float
                    "DeathStreakPenalty",
                    "MinDeathStreakPenalty";
                int "MinDeathsForStreak";
            }
            ["KillStreak"] {
                float
                    "KillStreakBonus",
                    "MaxKillStreakBonus";
                int "MinKillsForStreak";
            }
            ["Assist"] {
                float "GoldPoolForAssist";
            }
        );
    }
}

// DATA/Globals/ColorSettings.inibin
mod globals_colorsettings {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for team in ["Order", "Chaos"] {
                ["Color" team] {
                    for c in ["Red", "Green", "Blue"] {
                        float ("PlayerColorHighlight" c);
                    }
                }
            }
        );
    }
}

// DATA/Globals/Critical.inibin
mod globals_critical {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Karma"] {
                seq float "Critical" from 0;
            }
        );
    }
}

// DATA/Globals/GameMutatorExpansions.inibin
mod globals_gamemutatorexpansions {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for m in [
                "Default", "6v6", "ASCENSION", "BASICTUTORIAL", "Bilgewater", "PostBilgewater",
                "COF", "URF", "Clarity", "NoClarity", "DeathEvent", "ToggleDeathEvent",
                "UseOldDeathEvent", "ESPORT", "HEXAKILL", "HUD2015", "KingPoro", "NightmareBots",
                "NightmareBots1", "NightmareBots2", "NightmareBots3", "NightmareBotsDebug",
                "ONEFORALL", "OneForAllX", "OnboardCoopBeginner", "PoolPartyMinions", "TUTORIAL",
                "TagTeam",
            ] {
                [m] {
                    seq string "Mutator" from 1;
                }
            }
        );
    }
}

// DATA/Globals/HealthGain.inibin
mod globals_healthgain {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Coefficients"] {
                float
                    "MCoefficient",
                    "NCoefficient";
            }
        );
    }
}

// DATA/Globals/PingLimit.inibin
mod globals_pinglimit {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Threshold"] {
                float "StreakTime";
                int "LimitNumber";
            }
        );
    }
}

// DATA/Globals/Quests.inibin
mod globals_quests {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn area(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "TitleText";
            int "MaxViewable";
            for e in ["Recieved", "Completed", "Failed"] {
                string
                    (e "SoundEffect"),
                    (e "Text");
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for a in [
                "PrimaryQuests", "SecondaryQuests", "Objectives", "CharacterQuest_Corki",
                "ScoreQuestDisplay", "NEW_CONTENT_NOTIFICATION",
            ] {
                [a] {
                    use area();
                }
            }
        );
    }
}

// DATA/Globals/*_Stats.inibin
mod globals_stats {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["HeroStats"] {
                string
                    "ID",
                    "NAME",
                    "SKIN",
                    "TEAM",
                    "WIN",
                    "LEVEL",
                    "EXP",
                    "PING",
                    "KEYSTONE_ID",
                    "PLAYER_POSITION",
                    "PLAYER_ROLE",
                    "TIME_PLAYED";
                seq string "ITEM" from 0;
                for i in 1..5 {
                    string ("SPELL" i "_CAST");
                }
                for i in 1..3 {
                    string ("SUMMON_SPELL" i "_CAST");
                }
                seq string "PLAYER_SCORE_" from 0;
                string
                    "CHAMPIONS_KILLED",
                    "NUM_DEATHS",
                    "ASSISTS",
                    "DOUBLE_KILLS",
                    "TRIPLE_KILLS",
                    "QUADRA_KILLS",
                    "PENTA_KILLS",
                    "UNREAL_KILLS",
                    "KILLING_SPREES",
                    "LARGEST_KILLING_SPREE",
                    "LARGEST_MULTI_KILL",
                    "LARGEST_CRITICAL_STRIKE",
                    "BOUNTY_LEVEL",
                    "MINIONS_KILLED",
                    "NEUTRAL_MINIONS_KILLED",
                    "NEUTRAL_MINIONS_KILLED_YOUR_JUNGLE",
                    "NEUTRAL_MINIONS_KILLED_ENEMY_JUNGLE",
                    "SUPER_MONSTER_KILLED",
                    "TURRETS_KILLED",
                    "BARRACKS_KILLED",
                    "HQ_KILLED",
                    "FRIENDLY_TURRET_LOST",
                    "FRIENDLY_DAMPEN_LOST",
                    "FRIENDLY_HQ_LOST";
                string
                    "GOLD_EARNED",
                    "GOLD_SPENT",
                    "ITEMS_PURCHASED",
                    "CONSUMABLES_PURCHASED",
                    "SIGHT_WARDS_BOUGHT_IN_GAME",
                    "VISION_WARDS_BOUGHT_IN_GAME",
                    "WARD_PLACED",
                    "WARD_PLACED_DETECTOR",
                    "WARD_KILLED";
                for d in ["PHYSICAL", "MAGIC", "TRUE"] {
                    string
                        (d "_DAMAGE_DEALT_PLAYER"),
                        (d "_DAMAGE_DEALT_TO_CHAMPIONS"),
                        (d "_DAMAGE_TAKEN");
                }
                string
                    "TOTAL_DAMAGE_DEALT",
                    "TOTAL_DAMAGE_DEALT_TO_CHAMPIONS",
                    "TOTAL_DAMAGE_TAKEN",
                    "TOTAL_HEAL",
                    "TOTAL_UNITS_HEALED",
                    "TOTAL_TIME_CROWD_CONTROL_DEALT",
                    "TOTAL_TIME_SPENT_DEAD",
                    "LONGEST_TIME_SPENT_LIVING";
                string
                    "NODE_CAPTURE",
                    "NODE_CAPTURE_ASSIST",
                    "NODE_NEUTRALIZE",
                    "NODE_NEUTRALIZE_ASSIST",
                    "TEAM_OBJECTIVE",
                    "COMBAT_PLAYER_SCORE",
                    "OBJECTIVE_PLAYER_SCORE",
                    "TOTAL_PLAYER_SCORE",
                    "TOTAL_SCORE_RANK",
                    "VICTORY_POINT_TOTAL";
                string
                    "WAS_AFK",
                    "WAS_AFK_AFTER_FAILED_SURRENDER",
                    "NEVER_ENTERED_GAME",
                    "TEAMMATE_NEVER_ENTERED_GAME",
                    "TIME_SPENT_DISCONNECTED",
                    "TIME_OF_FROM_LAST_DISCONNECT",
                    "GAME_ENDED_IN_SURRENDER",
                    "GAME_ENDED_IN_EARLY_SURRENDER",
                    "TEAM_EARLY_SURRENDERED",
                    "WAS_EARLY_SURRENDER_ACCOMPLICE";
            }
        );
    }
}

// DATA/Globals/Tips.inibin
mod globals_tips {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn tips(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            int "MaxViewable";
            for k in ["TipRecievedSound", "DialogueOpenedSound", "DialogueClosedSound"] {
                string k;
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Global"] {
                use tips();
            }
            ["TUTORIAL"] {
                use tips();
            }
        );
    }
}

// DATA/Globals/UnitStats.inibin
mod globals_unitstats {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["MoveSpeed"] {
                for p in ["Slow", "Minor", "Major"] {
                    float
                        (p "MovePenaltyThreshold"),
                        (p "MovePenaltyCoefficient");
                }
                float "WallPathingFactor";
            }
        );
    }
}

// DATA/Characters/HeroSpawnOffsets.inibin
mod herospawnoffsets {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for team in ["Order", "Chaos"] {
                for n in 1..7 {
                    [team n] {
                        for i in (1..n + 1) {
                            vec3 ("Pos" i);
                            int ("Facing" i);
                        }
                    }
                }
            }
        );
    }
}

// DATA/Items/metadata/*.inibin
mod item_categories {
    use crate::reader::{Parts, Reader};
    use crate::recipes::{name, recipe};

    fn tags(r: &mut Reader, heading: Parts<'_>, names: &[&str]) {
        recipe!(r =>
            [heading] {
                int "_sortindex";
                for n in names {
                    int n;
                }
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            use tags(name!("Start"), &["Jungle", "Lane"]);
            use tags(name!("Tools"), &["Consumable", "GoldPer", "Vision"]);
            use tags(name!("Defense"), &[
                "Armor", "Health", "HealthRegen", "SpellBlock", "Tenacity",
            ]);
            use tags(name!("Attack"), &["AttackSpeed", "CriticalStrike", "Damage", "LifeSteal"]);
            use tags(name!("Magic"), &[
                "CooldownReduction", "Mana", "ManaRegen", "SpellDamage", "SpellVamp",
            ]);
            use tags(name!("Movement"), &["Boots", "NonbootsMovement"]);
            use tags(name!("Uncategorized"), &[
                "Active", "ArmorPenetration", "Aura", "MagicPenetration", "OnHit", "Slow",
                "SpellVamp", "Stealth", "Tenacity", "Trinket",
            ]);
        );
    }
}
