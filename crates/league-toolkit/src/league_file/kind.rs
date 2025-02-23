#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum LeagueFileKind {
    Animation,
    Jpeg,
    LightGrid,
    LuaObj,
    MapGeometry,
    Png,
    Preload,
    PropertyBin,
    PropertyBinOverride,
    RiotStringTable,
    SimpleSkin,
    Skeleton,
    StaticMeshAscii,
    StaticMeshBinary,
    Svg,
    Texture,
    TextureDds,
    Unknown,
    WorldGeometry,
    WwiseBank,
    WwisePackage,
}

impl LeagueFileKind {
    #[inline]
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Animation => "anm",
            Self::Jpeg => "jpg",
            Self::LightGrid => "lightgrid",
            Self::LuaObj => "luaobj",
            Self::MapGeometry => "mapgeo",
            Self::Png => "png",
            Self::Preload => "preload",
            Self::PropertyBin => "bin",
            Self::PropertyBinOverride => "bin",
            Self::RiotStringTable => "stringtable",
            Self::SimpleSkin => "skn",
            Self::Skeleton => "skl",
            Self::StaticMeshAscii => "sco",
            Self::StaticMeshBinary => "scb",
            Self::Texture => "tex",
            Self::TextureDds => "dds",
            Self::Unknown => "",
            Self::WorldGeometry => "wgeo",
            Self::WwiseBank => "bnk",
            Self::WwisePackage => "wpk",
            Self::Svg => "svg",
        }
    }

    #[must_use]
    pub fn from_extension(extension: impl AsRef<str>) -> LeagueFileKind {
        let extension = extension.as_ref();
        if extension.is_empty() {
            return LeagueFileKind::Unknown;
        }

        let extension = match extension.starts_with('.') {
            true => &extension[1..],
            false => extension,
        };

        match extension {
            "anm" => Self::Animation,
            "bin" => Self::PropertyBin,
            "bnk" => Self::WwiseBank,
            "dds" => Self::TextureDds,
            "jpg" => Self::Jpeg,
            "luaobj" => Self::LuaObj,
            "mapgeo" => Self::MapGeometry,
            "png" => Self::Png,
            "preload" => Self::Preload,
            "scb" => Self::StaticMeshBinary,
            "sco" => Self::StaticMeshAscii,
            "skl" => Self::Skeleton,
            "skn" => Self::SimpleSkin,
            "stringtable" => Self::RiotStringTable,
            "svg" => Self::Svg,
            "tex" => Self::Texture,
            "wgeo" => Self::WorldGeometry,
            "wpk" => Self::WwisePackage,
            _ => Self::Unknown,
        }
    }
}
