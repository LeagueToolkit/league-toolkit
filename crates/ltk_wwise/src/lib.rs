//! Reading, editing and writing the Wwise audio files of League of Legends.
//!
//! A `BankUnit` in a bin names three kinds of file:
//!
//! - a media bank (`*_audio.bnk`), which holds WEM media by source ID;
//! - an event bank (`*_events.bnk`), which holds the Wwise hierarchy of events, actions and sounds;
//! - a package (`*.wpk`), Riot's container of streamed WEM media.
//!
//! The crate reads each file into an owned value and writes the value back. A file that the
//! caller does not edit writes back byte for byte. The crate does not decode audio.
//!
//! `docs/design/wwise-audio.md` specifies the surface.

mod error;
mod ids;

pub mod bank;
pub mod hierarchy;
pub mod media;
pub mod package;
pub mod path;
pub mod upgrade;
pub mod wem;

pub use bank::Bank;
pub use error::{Error, Result};
pub use ids::{BankId, ObjectId, SourceId};
pub use media::MediaIndex;
pub use package::{MountedPackage, Package};
