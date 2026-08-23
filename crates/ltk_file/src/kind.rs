use std::io;

use super::pattern::{LEAGUE_FILE_MAGIC_BYTES, MAX_MAGIC_SIZE};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, strum::EnumIter)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
/// The kind of league file (animation, mapgeo, bin, etc)
pub enum LeagueFileKind {
    Animation,
    Jpeg,
    LightGrid,
    LuaObj,
    MapGeometry,
    Png,
    Tga,
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
    /// Returns an iterator over all [`LeagueFileKind`] variants.
    pub fn iter() -> impl Iterator<Item = Self> {
        <Self as strum::IntoEnumIterator>::iter()
    }

    #[inline]
    #[must_use]
    /// The extension for this file type (anm, mapgeo, bin, etc)
    /// ```
    /// # use ltk_file::LeagueFileKind;
    /// assert_eq!(LeagueFileKind::Animation.extension(), Some("anm"));
    /// assert_eq!(LeagueFileKind::StaticMeshAscii.extension(), Some("sco"));
    /// assert_eq!(LeagueFileKind::Unknown.extension(), None);
    ///
    pub fn extension(&self) -> Option<&'static str> {
        Some(match self {
            Self::Unknown => return None,
            Self::Animation => "anm",
            Self::Jpeg => "jpg",
            Self::LightGrid => "lightgrid",
            Self::LuaObj => "luaobj",
            Self::MapGeometry => "mapgeo",
            Self::Png => "png",
            Self::Tga => "tga",
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
            Self::WorldGeometry => "wgeo",
            Self::WwiseBank => "bnk",
            Self::WwisePackage => "wpk",
            Self::Svg => "svg",
        })
    }

    #[must_use]
    /// Infer the file type from the extension. Works with or without a preceding `'.'`.
    /// ```
    /// # use ltk_file::LeagueFileKind;
    /// #
    /// assert_eq!(LeagueFileKind::from_extension("png"), LeagueFileKind::Png);
    /// assert_eq!(LeagueFileKind::from_extension(".png"), LeagueFileKind::Png);
    /// ```
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
            "tga" => Self::Tga,
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

    /// Identify the type of league file from the magic at the start of the file. You must provide at
    /// least [`super::MAX_MAGIC_SIZE`] bytes of data to be able to detect all file types.
    ///
    /// # Examples
    /// ```
    /// # use ltk_file::*;
    /// #
    /// let data = b"r3d2skltblahblahblahblah";
    /// let kind = LeagueFileKind::identify_from_bytes(data);
    /// assert_eq!(kind, LeagueFileKind::Skeleton);
    /// ```
    ///
    ///
    /// ## Identifying from a reader
    /// ```
    /// # use std::fs::File;
    /// # use std::io::{self, Cursor, Read};
    /// # use ltk_file::*;
    /// #
    /// let mut reader = Cursor::new([0x33, 0x22, 0x11, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]);
    /// let mut buffer = [0; MAX_MAGIC_SIZE];
    /// reader.read(&mut buffer)?;
    ///
    /// let kind = LeagueFileKind::identify_from_bytes(&buffer);
    /// assert_eq!(kind, LeagueFileKind::SimpleSkin);
    /// # Ok::<(), io::Error>(())
    /// ```
    pub fn identify_from_bytes(data: &[u8]) -> LeagueFileKind {
        for magic_byte in LEAGUE_FILE_MAGIC_BYTES.iter() {
            if magic_byte.matches(data) {
                return magic_byte.kind;
            }
        }

        LeagueFileKind::Unknown
    }

    /// Identify the type of league file from the magic at the start of a reader, leaving the
    /// reader where it was.
    ///
    /// Reads at most [`super::MAX_MAGIC_SIZE`] bytes and seeks back, so the reader can be handed
    /// straight to whichever parser this names. A file shorter than that is fine; it is matched
    /// against the magics that fit.
    ///
    /// This is what an extension cannot tell you: `.bin` is both
    /// [`LeagueFileKind::PropertyBin`] and [`LeagueFileKind::PropertyBinOverride`].
    ///
    /// # Examples
    /// ```
    /// # use std::io::{self, Cursor};
    /// # use ltk_file::*;
    /// #
    /// let mut reader = Cursor::new(b"PTCH\x01\0\0\0PROP");
    /// let kind = LeagueFileKind::identify_from_reader(&mut reader)?;
    ///
    /// assert_eq!(kind, LeagueFileKind::PropertyBinOverride);
    /// assert_eq!(reader.position(), 0);
    /// # Ok::<(), io::Error>(())
    /// ```
    pub fn identify_from_reader<R: io::Read + io::Seek + ?Sized>(
        reader: &mut R,
    ) -> io::Result<LeagueFileKind> {
        let start = reader.stream_position()?;

        let mut magic = [0; MAX_MAGIC_SIZE];
        let read = read_up_to(reader, &mut magic)?;
        reader.seek(io::SeekFrom::Start(start))?;

        Ok(Self::identify_from_bytes(&magic[..read]))
    }
}

/// Fills as much of `buffer` as the reader has left, unlike `read_exact`, which fails short.
fn read_up_to<R: io::Read + ?Sized>(reader: &mut R, buffer: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buffer.len() {
        match reader.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(read) => filled += read,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek as _};

    use super::*;

    /// The magic wins over the extension: both kinds of bin file are `.bin`.
    #[test]
    fn identifies_both_kinds_of_bin() {
        assert_eq!(
            LeagueFileKind::identify_from_bytes(b"PROP\x03\0\0\0"),
            LeagueFileKind::PropertyBin
        );
        assert_eq!(
            LeagueFileKind::identify_from_bytes(b"PTCH\x01\0\0\0"),
            LeagueFileKind::PropertyBinOverride
        );
        assert_eq!(
            LeagueFileKind::from_extension("bin"),
            LeagueFileKind::PropertyBin
        );
        assert_eq!(LeagueFileKind::PropertyBin.extension(), Some("bin"));
        assert_eq!(LeagueFileKind::PropertyBinOverride.extension(), Some("bin"));
    }

    #[test]
    fn identifies_from_a_reader_without_moving_it() {
        let mut reader = Cursor::new(b"PTCH\x01\0\0\0PROP\x03\0\0\0");
        assert_eq!(
            LeagueFileKind::identify_from_reader(&mut reader).unwrap(),
            LeagueFileKind::PropertyBinOverride
        );
        assert_eq!(reader.position(), 0);

        // Wherever the reader is, that is where the magic is read and where it is left.
        reader.seek(std::io::SeekFrom::Start(8)).unwrap();
        assert_eq!(
            LeagueFileKind::identify_from_reader(&mut reader).unwrap(),
            LeagueFileKind::PropertyBin
        );
        assert_eq!(reader.position(), 8);
    }

    #[test]
    fn identifies_a_file_shorter_than_the_longest_magic() {
        let mut reader = Cursor::new(b"OEGM");
        assert_eq!(
            LeagueFileKind::identify_from_reader(&mut reader).unwrap(),
            LeagueFileKind::MapGeometry
        );

        let mut reader = Cursor::new(b"");
        assert_eq!(
            LeagueFileKind::identify_from_reader(&mut reader).unwrap(),
            LeagueFileKind::Unknown
        );
    }

    #[test]
    fn reports_an_unknown_magic() {
        assert_eq!(
            LeagueFileKind::identify_from_bytes(b"nope nope nope"),
            LeagueFileKind::Unknown
        );
        assert_eq!(
            LeagueFileKind::identify_from_reader(&mut Cursor::new(b"nope nope nope")).unwrap(),
            LeagueFileKind::Unknown
        );
    }
}
