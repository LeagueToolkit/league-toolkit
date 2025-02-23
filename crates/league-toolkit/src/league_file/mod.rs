mod kind;
mod pattern;

pub use kind::*;
use pattern::LEAGUE_FILE_MAGIC_BYTES;
pub use pattern::MAX_MAGIC_SIZE;

/// Identify the type of league file from the magic at the start of the file. You must provide at
/// least [`struct@MAX_MAGIC_SIZE`] bytes of data to be able to detect all file types.
pub fn identify_league_file(data: &[u8]) -> LeagueFileKind {
    for magic_byte in LEAGUE_FILE_MAGIC_BYTES.iter() {
        if magic_byte.matches(data) {
            return magic_byte.kind;
        }
    }

    LeagueFileKind::Unknown
}
