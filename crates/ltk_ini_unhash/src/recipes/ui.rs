//! The recipe of interface files: loading screens, the HUD, unit bars and game bar animations.

use crate::reader::Reader;

pub(crate) fn read(r: &mut Reader) {
    gamebar_animation::read(r);
    loadingscreen::read(r);
    menu_generalcharacterdata::read(r);
    menu_hud::read(r);
    menu_parstates::read(r);
    menu_unitbardata::read(r);
}

// DATA/Images/UI/GameBar/*/*.inibin
mod gamebar_animation {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Animation"] {
                float "AnimationTime";
            }
            ["Animation"] {
                if let n = int "FrameCount" {
                    ["Frames"] {
                        for i in (0..n) {
                            string ("Frame" i);
                        }
                    }
                }
            }
        );
    }
}

// DATA/LoadingScreen/**/*.inibin
mod loadingscreen {
    use crate::reader::{Parts, Reader, Sec};
    use crate::recipes::{name, recipe};

    fn box_(r: &mut Reader, s: Sec, prefix: Parts<'_>) {
        recipe!(r, s =>
            for k in ["X", "Y", "Width", "Height"] {
                int (prefix k);
            }
        );
    }

    fn rect(r: &mut Reader, s: Sec, prefix: Parts<'_>) {
        recipe!(r, s =>
            for k in ["U", "V", "BotU", "BotV", "ChaosOffset"] {
                float (prefix k);
            }
        );
    }

    fn font(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "Font",
                "FontResourceFile";
            int
                "FontSize",
                "FontOutlineSize";
            int "Bold";
            vec3
                "ColorIn",
                "ColorOut",
                "FontColorIn",
                "FontColorOut";
        );
    }

    fn team_colors(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            for side in ["Order", "Chaos"] {
                vec3
                    ("ColorIn" side),
                    ("ColorOut" side);
            }
        );
    }

    fn background(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "Image";
            int "EnableVs";
            int
                "ImageWidth",
                "ImageHeight";
            string
                "Icon",
                "IconOrder",
                "IconChaos";
        );
    }

    fn badges(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            for i in 0..4 {
                int
                    ("Badge" i "_ID"),
                    ("Badge" i "_ID_Old"),
                    ("Badge" i "TexU"),
                    ("Badge" i "TexV");
            }
            int
                "BadgeTexWidth",
                "BadgeTexHeight";
        );
    }

    fn data(r: &mut Reader, d: Sec) {
        recipe!(r, d =>
            use background();
            use badges();
            string "AscensionBackground";
            use rect(name!("BackgroundTex"));
            use rect(name!("ProgressbarBorderTex"));
            use rect(name!("ProgressbarTex"));
            use rect(name!("VsBorderTex"));
            int
                "BadgeBorderTexU",
                "BadgeBorderTexV",
                "BadgeBorderTexWidth",
                "BadgeBorderTexHeight";
            for i in 0..4 {
                for k in ["u1", "u2", "v1", "v2"] {
                    int ("BarTextUVs" i k);
                }
            }
            for b in [
                "BlackBox", "ChampionBox", "ChampionTextBox", "KeystoneBox", "PerkStyle",
                "PerkSubStyle",
            ] {
                use box_(name!(b));
            }
            int
                "SummonorBoxWidth",
                "SummonorBoxHeight",
                "SummonorSpellBoxX",
                "SummonorSpellBox2X",
                "SummonorSpellBoxY";
            float "PerkSubStyleIconScale";
            for k in ["X", "Y", "Width", "Height"] {
                int ("CardBar" k "Factor");
            }
            int
                "CardWidthFixed",
                "CardHeightFixed";
            int "ChampionCardHorizontalLayout";
            float "ChampionCardHeightPercentage";
            for side in ["Order", "Chaos"] {
                for k in ["X", "Y", "Width", "Height"] {
                    float (side "Card" k "Percentage");
                }
            }
            float
                "PercentageMarginBetweenCardsX",
                "PercentageOfScreenTopMargin",
                "PercentageOfScreenMiddleMargin",
                "PercentageOfScreenBottomMargin";
            for p in ["PingBox", "PingFixedBox"] {
                int
                    (p "HeightFactor"),
                    (p "OffsetFactorX"),
                    (p "OffsetFactorY");
                float (p "WidthFactor");
            }
            float
                "PingBarMarginX",
                "PingBarMarginY";
            int
                "ProfileIconWidthFactor",
                "ProfileIconHeightFactor",
                "ProfileIconOffsetFactorX",
                "ProfileIconOffsetFactorY";
            for sfx in ["", "New"] {
                string ("RankingsTextureAtlas" sfx);
                int
                    ("RankingsTextureX" sfx),
                    ("RankingsTextureY" sfx);
            }
            string
                "MessageOffsetX",
                "MessageOffsetY",
                "PercentageCompleteTextX",
                "PercentageCompleteTextY";
            int
                "PercentageCompleteNoPingTextY",
                "ChampionNameHeight",
                "ChampionNameHeightForLongString",
                "ChampionNameHeightHeightForLongString",
                "LongChampionName",
                "SummonerNameHeight",
                "PercentNameHeight",
                "TeamsTextHeight",
                "TipsTextHeight",
                "TextWidth",
                "VsWidthFactor",
                "VsHeightFactor",
                "DivisionMarkerYOffset",
                "DebugPing";
            float
                "ChampionNameOffset",
                "SummonerNameOffset",
                "ProgressTextX",
                "ProgressTextY";
            int
                "UseSkinNames",
                "LiveUpdate",
                "DebugNumberOnOrder",
                "DebugNumberOnChaos",
                "DebugNumberOnChoas";
            string
                "HonorMasteryTextureAtlasSettings",
                "TextureName";
            int
                "TextureWidth",
                "TextureHeight",
                "Width",
                "Height";
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Data"] {
                use data();
            }
            [""] {
                use data();
            }
            ["Data"] {
                if let n = int "Textures" {
                    for i in (0..n) {
                        ["Texture" i] {
                            string "Name";
                            int
                                "Left",
                                "Top",
                                "Width",
                                "Height";
                        }
                    }
                }
            }
            ["Background"] {
                use background();
                int
                    "ActualSizeWidth",
                    "ActualSizeHeight",
                    "LoadBarWidth",
                    "LoadBarHeight",
                    "LoadBarImageWidth",
                    "LoadBarImageHeight",
                    "LeftLoadBarImageWidth",
                    "LeftLoadBarImageHeight",
                    "RightLoadBarImageWidth",
                    "RightLoadBarImageHeight";
                float
                    "LetterBoxBorderPercentage",
                    "LoadBarWidthPercent",
                    "LoadBarHeightPercent",
                    "PercentLoadBarFromBottom";
                int
                    "SlowAutoReload",
                    "ShowLoadBarSideImages";
            }
            for i in 1..130 {
                ["Map" i] {
                    use background();
                }
            }
            for sec in [
                "FIRSTBLOOD", "TFT", "AscensionBackground", "CRDarkStarBackground",
                "HABilgewaterBackground", "KingPoroBackground", "OdysseyBackground",
                "ProjectMapBackground", "SRArcadeBackground", "SRBloodMoonBackground",
                "SRPoolPartyBackground", "SRSnowdownBackground", "SRVersusBackground",
                "StarGuardianBackground", "URFBackground", "WIPMODEWIP4Background",
            ] {
                [sec] {
                    use background();
                }
            }
            ["NightmareBots"] {
                use background();
                string "DifficultyIconImage";
                int
                    "BotDifficultyWidth",
                    "BotDifficultyHeight",
                    "BotDifficultySpacing",
                    "GameModeTextHeight";
                float
                    "BotDifficultyX",
                    "BotDifficultyY",
                    "GameModeTextX",
                    "GameModeTextY";
            }
            ["FirstBlood"] {
                float
                    "HBuffer",
                    "HOffset";
                int "VOffset";
            }
            for sec in [
                "Large", "Small", "SmallForLongSkinNames", "Progress", "Teams", "Tips", "Ping",
                "VS", "GameMode", "ObjectiveText", "Title",
            ] {
                [sec] {
                    use font();
                }
            }
            ["Large"] {
                vec3
                    "ClubTagColorIn",
                    "ClubTagColorOut";
            }
            ["Small"] {
                use team_colors();
            }
            ["Teams"] {
                use team_colors();
                for side in ["Order", "Chaos"] {
                    int
                        ("Region" side "X"),
                        ("Region" side "Y");
                }
            }
            ["Progress"] {
                float
                    "ForGroundProgressBarXOffset",
                    "TextOffsetX",
                    "TextOffsetY",
                    "TextOffsetXShadow",
                    "TextOffsetYShadow";
            }
            ["Ping"] {
                for c in ["Green", "Yellow", "Orange", "Red"] {
                    vec3 ("ColorIn" c);
                    int ("PingThreshold" c);
                }
            }
            ["VS"] {
                int "Scale";
                float
                    "XPosPercentOfScreen",
                    "YPosPercentOfScreen";
            }
            ["Tips"] {
                string "TipBackground";
            }
            ["ObjectiveText"] {
                int "TimeBetweenMessagesSeconds";
                for i in 1..6 {
                    string
                        ("Objective" i "Line1"),
                        ("Objective" i "Line2");
                }
            }
            ["Title"] {
                string "Text";
                float
                    "TitleScreenOffsetXPercent",
                    "TitleScreenOffsetYPercent",
                    "TitleOffScreenOffsetXPercent";
            }
            for side in ["Left", "Right", "PBI"] {
                ["Tips" side] {
                    int
                        "RegionX",
                        "RegionY",
                        "RegionWidth",
                        "RegionHeight";
                    string
                        "TipsType",
                        "TipsListFile",
                        "TipsListSection";
                }
            }
            ["ChampionMastery"] {
                if let n = int "NumMasteryIcons" {
                    for i in (0..n) {
                        int ("MasteryIcon" i "Level");
                        string ("MasteryIcon" i "Name");
                    }
                }
                int
                    "MasteryIconXOffset",
                    "MasteryIconYOffset";
            }
            ["Honor"] {
                use badges();
                int
                    "NumHonorIcons",
                    "BadgeYOffset";
            }
        );
    }
}

// DATA/Menu*/GeneralCharacterData*.inibin
// DATA/Characters/GeneralCharacterData*.inibin
mod menu_generalcharacterdata {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    const MODES: &[&str] = &["Colorblind", "Default", "Spectator"];

    const ALL_MODES: &[&str] = &["", "Colorblind", "Default", "Spectator"];

    const TEAMS: &[&str] = &["Enemy", "Friendly", "Self"];

    fn bar_template(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "BarTexture";
            int "TickPixelWidth";
            vec4
                "BarPixelRegion",
                "MegaTickPixelRegion",
                "TickPixelRegion",
                "HighlightMidPixelRegion";
        );
    }

    fn backdrop_template(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "BackgroundTexture";
            for part in ["Left", "Mid", "Right"] {
                vec4
                    ("Background" part "PixelRegion"),
                    ("Highlight" part "PixelRegion");
            }
        );
    }

    fn text_template(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "FontDescription",
                "FontName";
            int
                "FontSize",
                "DropShadowDepth",
                "OutlineSize";
        );
    }

    fn exp_bar_template(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "LevelUpAnimation",
                "ExeprienceAnimation",
                "ExperienceBar",
                "ExperienceBarMask";
            int
                "LevelUpAnimationOffsetX",
                "LevelUpAnimationOffsetY",
                "ExperienceAnimatinonOffsetX",
                "ExperienceAnimatinonOffsetY",
                "ExperienceBarOffsetX",
                "ExperienceBarOffsetY";
        );
    }

    fn loc_icons(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "AirborneTexture",
                "BlindTexture",
                "CharmTexture",
                "DisarmTexture",
                "FearTexture",
                "RootTexture",
                "SilenceTexture",
                "SlowTexture",
                "StunTexture",
                "SuppressionTexture",
                "TauntTexture";
            int
                "IconWidth",
                "IconHeight";
            float
                "IconOffsetX",
                "IconOffsetY",
                "AdditionalIconOffsetX",
                "AdditionalIconOffsetY";
        );
    }

    fn team_bar(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            for m in ALL_MODES {
                string
                    ("BackgroundTexture" m),
                    ("BarTexture" m);
                vec4 ("BarTint" m);
            }
            vec4
                "BackgroundPixelRegion",
                "BackgroundTint",
                "BarPixelRegion",
                "BarRectEndPoints",
                "TickPixelRegion";
            float
                "BarRepetitionX",
                "BarRepetitionY",
                "PercentageOffsetX",
                "PercentageOffsetY",
                "ScalerX",
                "ScalerY";
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for t in ["", "Enemy", "Friendly", "Self"] {
                for b in ["HealthBar", "LoCBar", "PARBar"] {
                    ["Champion" t b "Template"] {
                        use bar_template();
                    }
                }
            }
            ["ChampionHPMaxPenaltyTemplate"] {
                use bar_template();
            }
            for t in TEAMS {
                for m in MODES {
                    ["Champion" t "BackdropTemplate" m] {
                        use backdrop_template();
                    }
                }
                ["Champion" t "ExperienceBarTemplate"] {
                    use exp_bar_template();
                }
                ["Champion" t "LoCBarIconData"] {
                    use loc_icons();
                }
            }
            ["ChampionChatBubbleTemplate"] {
                use backdrop_template();
            }
            ["ChampionLoCBarBackdropTemplate"] {
                use backdrop_template();
            }
            for n in [
                "ChampionClubTagTextTemplate", "ChampionSelfClubTagTextTemplate",
                "ChampionLevelTextTemplate", "ChampionTitleTextTemplate",
                "ChampionSelfTitleTextTemplate",
            ] {
                [n] {
                    use text_template();
                }
            }
            for p in ["Unit", "NewUnit"] {
                for b in ["HealthBar", "PowerBar", "TimerBar"] {
                    [p b "Template"] {
                        use bar_template();
                    }
                }
                for b in ["Backdrop", "DoubleBackdrop"] {
                    [p b "Template"] {
                        use backdrop_template();
                    }
                }
            }
            ["UnitAggroBackdropTemplate"] {
                use backdrop_template();
            }
            for t in ["Enemy", "Friendly", "Neutral"] {
                for m in MODES {
                    ["Legacy" t "UnitHealthBarTemplate" m] {
                        use bar_template();
                    }
                    ["Legacy" t "UnitBackdropTemplate" m] {
                        use backdrop_template();
                    }
                }
            }
            for t in TEAMS {
                for m in MODES {
                    ["HealthBarChampion" t m] {
                        float "FadeSpeed";
                        int "TitleTextDropShadow";
                        vec2
                            "ChatBubbleOffset",
                            "HealthOffset",
                            "HPBarStartOffset",
                            "HPMaxPenaltyBarOffset",
                            "LevelTextOffset",
                            "LoCOffset",
                            "PAROffset",
                            "PercentageOffset",
                            "TitleTextOffset";
                        vec3
                            "LevelTextColor",
                            "TitleTextColor",
                            "TitleTextOutlineColor";
                        vec4
                            "AllShieldColor",
                            "HealthBarColor",
                            "HealthFadeColor",
                            "HealthFillColor",
                            "HPMaxPenaltyBarColor",
                            "MagicShieldColor",
                            "PhysicalShieldColor",
                            "PhysShieldColor";
                    }
                }
            }
            for p in ["", "New"] {
                for t in ["Enemy", "Friendly", "Neutral"] {
                    for m in MODES {
                        [p "HealthBar" t "Unit" m] {
                            float "MidHorizontalScaling";
                            vec2
                                "AggroIndicatorOffset",
                                "HPBarStartOffset",
                                "HPOnlyBackdropOffset",
                                "PowerBarStartOffset",
                                "TimerBarStartOffset";
                            vec4
                                "BackgroundColor",
                                "HealthBarColor",
                                "HighlightColor",
                                "PowerBarColor",
                                "TimerBarColor";
                        }
                    }
                }
            }
            for u in ["Inhibitor", "Minion", "Nexus", "Turret"] {
                ["LegacyHealthBar" u] {
                    float "MidHorizontalScaling";
                    vec2
                        "BarStartOffset",
                        "BarEndOffset",
                        "PercentageOffset";
                    vec4
                        "BackgroundColor",
                        "BarColor",
                        "FadeColor";
                }
            }
            for u in ["Dampener", "HQ", "Minion", "Turret"] {
                for t in ["Chaos", "Order", "Neutral"] {
                    ["HealthBar" u t "Team"] {
                        use team_bar();
                    }
                }
            }
            ["HealthBarDampenerChaosTeam"] {
                float "DefaultChampionCollisionRadius";
            }
            for t in ["Chaos", "Order"] {
                for w in ["Player", "Team"] {
                    ["HealthBarHero" t w] {
                        use team_bar();
                        use exp_bar_template();
                        string
                            "BackgroundTextureHighlight",
                            "ChatBubble",
                            "LevelFontName",
                            "QuoteBoneId";
                        int "DrawLevel";
                        int
                            "FadeSpeed",
                            "LevelFontSize",
                            "LevelX",
                            "LevelY",
                            "ManaBarYOffset",
                            "ParallaxOffset";
                        vec2 "ChatBubbleOffset";
                        vec4
                            "FadeColor",
                            "LevelColor",
                            "EnergyBarPixelRegion",
                            "EnergyFadeColor",
                            "ManaBarPixelRegion",
                            "ManaBarRectEndPoints",
                            "ManaBarTint",
                            "ManaTickPixelRegion",
                            "MPBarPixelRegion",
                            "MPFadeColor",
                            "ShieldBarPixelRegion",
                            "ShieldFadeColor",
                            "PhysShieldTint",
                            "PhysAndMagicShieldTint";
                        for m in ALL_MODES {
                            vec4 ("MagicShieldTint" m);
                        }
                        int
                            "QuoteWidth",
                            "QuoteHeight",
                            "QuoteRightFromHead",
                            "QuoteDownFromHead",
                            "QuoteAlphaTop",
                            "QuoteAlphaBottom";
                        float
                            "QuoteLeft",
                            "QuoteTopOffset";
                        vec4 "QuoteColor";
                    }
                }
            }
            ["GeneralDataHero"] {
                int
                    "ChampionParallaxOffset",
                    "DefaultHealthPerMegaTick",
                    "DefaultHealthPerTick",
                    "MaxHealthTicks",
                    "FullHealthAlpha",
                    "ZeroHealthAlpha";
                float
                    "DefaultBoundingCylinderHeight",
                    "DefaultBoundingCylinderRadius",
                    "DefaultBoundingSphereRadius",
                    "DefaultChampionCollisionRadius",
                    "FadeTimerForHealthBar";
                vec4
                    "EnergyBarColor",
                    "EnergyFadeColor",
                    "MPBarColor",
                    "MPFadeColor",
                    "OtherBarColor",
                    "OtherFadeColor",
                    "ShieldBarColor",
                    "ShieldFadeColor";
            }
            ["HealthBarSettings"] {
                int
                    "GoTransparent",
                    "UseCompression";
                int
                    "DefaultHealthPerMegaTick",
                    "DefaultHealthPerTick",
                    "DefaultHealthPerMicroTick",
                    "MaxHealthTicks",
                    "MaxHealthMicroTicks",
                    "TickAlpha",
                    "MegaTickAlpha",
                    "MicroTickAlpha";
                float
                    "TickThickness",
                    "MegaTickThickness",
                    "MicroTickThickness",
                    "MicroTickHeight";
            }
            ["HealthBarIconsSetting"] {
                int "SpaceBetweenIcons";
                vec2
                    "TopIconSize",
                    "TopCenterOffset",
                    "BotIconSize",
                    "BotLeftOffset";
            }
            ["UnitAggroEffect"] {
                string "TextureSheet";
                for i in 1..13 {
                    vec4 ("KeyFrame" i "_PixelRegion");
                    float ("KeyFrame" i "_Time");
                }
            }
            ["AttackData"] {
                float "AttackAutoInterruptPercent";
            }
        );
    }
}

// DATA/Menu*/HUD.inibin
mod menu_hud {
    use crate::reader::Reader;
    use crate::recipes::recipe;

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            ["Position"] {
                int
                    "BaseOffsetFromEdge",
                    "LaneOffsetFromTopOfScreen",
                    "SizeOfTowerDisplay";
            }
            ["Texture"] {
                vec4
                    "BaseBorder",
                    "BaseFill",
                    "BaseIcon",
                    "TowerBorder",
                    "TowerFill",
                    "TowerIcon",
                    "Top",
                    "Mid",
                    "Bot",
                    "ButtonBG",
                    "IndicatorBG";
            }
        );
    }
}

// DATA/Menu*/PARStates.inibin
mod menu_parstates {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;

    fn par_type(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            int
                "ShowPAR",
                "ShowRegen";
            let num_states = int "NumStates";
            for i in (0..num_states.unwrap_or(1)) {
                vec4
                    ("State" i "Color"),
                    ("State" i "FadeColor");
                string ("State" i "VideoPrefix");
            }
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for p in [
                "None", "MP", "Energy", "Shield", "Other", "BattleFury", "DragonFury", "Rage",
                "Heat", "GnarFury", "Ferocity", "BloodWell", "Wind",
            ] {
                [p] {
                    use par_type();
                }
            }
            ["GnarFury"] {
                stale {
                    vec4
                        "State5Color",
                        "State5FadeColor";
                    string "State5VideoPrefix";
                }
            }
        );
    }
}

// DATA/Menu*/UnitBarData.inibin
mod menu_unitbardata {
    use crate::reader::{Reader, Sec};
    use crate::recipes::recipe;
    use crate::Ty;

    fn backdrop(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "BackgroundTexture";
            vec4
                "BackgroundLeftPixelRegion",
                "BackgroundMidPixelRegion",
                "BackgroundRightPixelRegion",
                "HighlightLeftPixelRegion",
                "HighlightMidPixelRegion",
                "HighlightRightPixelRegion";
        );
    }

    fn bar(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string "BarTexture";
            vec4
                "BarPixelRegion",
                "TickPixelRegion",
                "MegaTickPixelRegion";
            int "TickPixelWidth";
        );
    }

    fn text(r: &mut Reader, s: Sec) {
        recipe!(r, s =>
            string
                "FontName",
                "FontDescription";
            int "DropShadowDepth";
        );
    }

    fn params(r: &mut Reader, s: Sec, fade: Option<Ty>) {
        recipe!(r, s =>
            vec4
                "HealthBarColor",
                "HighlightColor",
                "BackgroundColor",
                "AllShieldColor",
                "PhysShieldColor",
                "MagicShieldColor",
                "PowerBarColor",
                "TimerBarColor";
            read (fade) "HealthFadeColor";
            vec2
                "HPBarStartOffset",
                "HealthTextOffset",
                "AggroIndicatorOffset",
                "PAROffset",
                "PowerBarStartOffset",
                "TimerBarStartOffset";
            float
                "FadeSpeed",
                "MidHorizontalScaling";
        );
    }

    pub(super) fn read(r: &mut Reader) {
        recipe!(r =>
            for u in [
                "UnitSingle", "UnitDouble", "Counter", "Turret", "TurretResource", "HATurret",
                "TTTurret", "InhibMage", "InhibPaladin", "NexusMage", "NexusPaladin", "HAInhib",
                "HANexus", "TTInhib", "TTNexus", "LargeMonster", "Dragon", "Baron", "Vilemaw",
                "Ward",
            ] {
                [u "_Backdrop"] {
                    use backdrop();
                }
                for b in ["HealthBar", "ParBar", "TimerBar"] {
                    [u "_" b] {
                        use bar();
                    }
                }
                [u "_HealthText"] {
                    use text();
                }
                for team in ["Enemy", "Friendly", "Neutral"] {
                    let ward = (u.eq_ignore_ascii_case("Ward") && !team.eq_ignore_ascii_case("Neutral"));
                    let fade = (Some(if ward { Ty::Vec3 } else { Ty::Vec4 }));
                    for mode in ["Default", "Colorblind", "Spectator"] {
                        [u "_UnitParams" team mode] {
                            use params(fade);
                        }
                    }
                }
            }
        );
    }
}
