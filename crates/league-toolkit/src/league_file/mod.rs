mod kind;
mod pattern;

pub use kind::*;
use pattern::LEAGUE_FILE_MAGIC_BYTES;

pub fn identify_league_file(data: &[u8]) -> LeagueFileKind {
    for magic_byte in LEAGUE_FILE_MAGIC_BYTES.iter() {
        if magic_byte.matches(data) {
            return magic_byte.kind;
        }
    }

    LeagueFileKind::Unknown
}
