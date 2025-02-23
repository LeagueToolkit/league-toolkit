mod kind;
mod pattern;

pub use kind::*;
use pattern::LEAGUE_FILE_MAGIC_BYTES;
pub use pattern::MAX_MAGIC_SIZE;

/// Identify the type of league file from the magic at the start of the file. You must provide at
/// least [`MAX_MAGIC_SIZE`] bytes of data to be able to detect all file types.
///
/// # Examples
/// ```
/// # use league_toolkit::league_file::*;
/// #
/// let data = b"r3d2skltblahblahblahblah";
/// let kind = identify_league_file(data);
/// assert_eq!(kind, LeagueFileKind::Skeleton);
/// ```
///
///
/// ## Identifying from a reader
/// ```
/// # use std::fs::File;
/// # use std::io::{self, Cursor, Read};
/// # use league_toolkit::league_file::*;
/// #
/// let mut reader = Cursor::new([0x33, 0x22, 0x11, 0x00, 0xDE, 0xAD, 0xBE, 0xEF]);
/// let mut buffer = [0; MAX_MAGIC_SIZE];
/// reader.read(&mut buffer)?;
///
/// let kind = identify_league_file(&buffer);
/// assert_eq!(kind, LeagueFileKind::SimpleSkin);
/// # Ok::<(), io::Error>(())
/// ```
pub fn identify_league_file(data: &[u8]) -> LeagueFileKind {
    for magic_byte in LEAGUE_FILE_MAGIC_BYTES.iter() {
        if magic_byte.matches(data) {
            return magic_byte.kind;
        }
    }

    LeagueFileKind::Unknown
}
