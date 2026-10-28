//! The Action layout.

use std::fmt;

use crate::{BankId, ObjectId};

use super::layout::{Generation, Put, Reader, Step, Stop};

/// The class of a Play action.
const PLAY_CLASS: u16 = 0x0400;

/// The fade curve of an action that [`Action::play`] returns.
const PLAY_FADE_CURVE: u8 = 4;

/// An action: one step that an event runs, such as Play, Stop or Set State.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub id: ObjectId,
    pub action_type: ActionType,
    /// The object that the action acts on: `idExt`.
    pub target: ObjectId,
    /// The flags of the target: `idExt_4`.
    pub target_flags: u8,
    /// The `PropBundle` and the `RangedPropBundle`.
    props: Vec<u8>,
    /// The extension of the class, without the field of the current generation.
    extension: Vec<u8>,
    /// The `u32` of the current generation after the extension of a Play action.
    play_tail: u32,
}

impl Action {
    /// Returns a Play action of `target` in the bank `bank`, with empty property bundles and
    /// fade curve 4.
    ///
    /// ```
    /// use ltk_wwise::{hierarchy::Action, BankId, ObjectId};
    ///
    /// let play = Action::play(ObjectId(1), ObjectId(2), BankId(3));
    /// assert!(play.action_type.is_play());
    /// assert_eq!(play.play_bank(), Some(BankId(3)));
    /// ```
    pub fn play(id: ObjectId, target: ObjectId, bank: BankId) -> Self {
        let mut extension = vec![PLAY_FADE_CURVE];
        extension.put_u32(bank.0);
        Self {
            id,
            action_type: ActionType::PLAY,
            target,
            target_flags: 0,
            props: vec![0, 0],
            extension,
            play_tail: 0,
        }
    }

    /// Returns the bank of a Play action, or `None` for an action of another class.
    pub fn play_bank(&self) -> Option<BankId> {
        if !self.action_type.is_play() {
            return None;
        }
        let [_, b0, b1, b2, b3] = self.extension[..] else {
            return None;
        };
        Some(BankId(u32::from_le_bytes([b0, b1, b2, b3])))
    }

    /// Reads an action payload.
    pub(super) fn read(reader: &mut Reader<'_>, generation: Generation) -> Step<Self> {
        let id = reader.id()?;
        let action_type = ActionType(reader.u16()?);
        let target = reader.id()?;
        let target_flags = reader.u8()?;
        let props = reader.bytes_of(|reader| {
            reader.prop_bundle()?;
            reader.ranged_prop_bundle()
        })?;
        let class = action_type.class();
        let extension = reader.bytes_of(|reader| read_extension(reader, class, generation))?;
        let play_tail = if class == PLAY_CLASS && generation.is_current() {
            reader.u32()?
        } else {
            0
        };
        Ok(Self {
            id,
            action_type,
            target,
            target_flags,
            props,
            extension,
            play_tail,
        })
    }

    /// Writes an action payload.
    pub(super) fn write(&self, out: &mut Vec<u8>, generation: Generation) {
        out.put_u32(self.id.0);
        out.put_u16(self.action_type.0);
        out.put_u32(self.target.0);
        out.put_u8(self.target_flags);
        out.extend_from_slice(&self.props);
        out.extend_from_slice(&self.extension);
        if self.action_type.is_play() && generation.is_current() {
            out.put_u32(self.play_tail);
        }
    }
}

/// Skips the extension of the class `class` that both generations share.
///
/// Stops with [`Stop::Class`] for a class outside the table of `generation`.
fn read_extension(reader: &mut Reader<'_>, class: u16, generation: Generation) -> Step<()> {
    match class {
        0x0100 | 0x0200 | 0x0300 | 0x1A00 | 0x1B00 => {
            reader.skip(2)?;
            reader.exceptions()
        }
        PLAY_CLASS => reader.skip(1 + 4),
        0x0600 | 0x0700 | 0x2200 => {
            reader.skip(1)?;
            reader.exceptions()
        }
        0x0800..=0x0F00 | 0x2000 | 0x3000 => {
            reader.skip(2 + 3 * 4)?;
            reader.exceptions()
        }
        0x1200 | 0x1900 => reader.skip(2 * 4),
        0x1300 | 0x1400 => {
            reader.skip(3 + 3 * 4)?;
            reader.exceptions()
        }
        0x1E00 => {
            reader.skip(1 + 3 * 4 + 1)?;
            reader.exceptions()
        }
        0x3100 | 0x3200 if generation.is_current() => {
            reader.skip(2 + 4 + 1)?;
            reader.exceptions()
        }
        0x1500 | 0x1600 | 0x1700 if !generation.is_current() => Ok(()),
        0x1000 | 0x1100 | 0x1C00 | 0x1D00 | 0x1F00 | 0x2100 => Ok(()),
        _ => Err(Stop::Class(class)),
    }
}

/// The `ulActionType` field of an action.
///
/// The high byte is the class. The low byte selects a variant of the class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActionType(pub u16);

impl ActionType {
    /// Play, `0x0403`.
    pub const PLAY: Self = Self(0x0403);

    /// Returns the class: `action_type & 0xFF00`.
    pub fn class(self) -> u16 {
        self.0 & 0xFF00
    }

    /// Returns `true` for the class `0x0400`, Play.
    pub fn is_play(self) -> bool {
        self.class() == PLAY_CLASS
    }
}

/// Writes the value in hexadecimal: `0x0403`.
impl fmt::Display for ActionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#06x}", self.0)
    }
}
