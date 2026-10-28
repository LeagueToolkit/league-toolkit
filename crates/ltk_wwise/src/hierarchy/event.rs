//! The Event layout.

use crate::{ObjectId, Result};

use super::layout::{Put, Reader, Step};

/// An event: the actions that the client runs when the game posts the event.
///
/// The ID of an event is the FNV-1 hash of its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    pub id: ObjectId,
    /// The IDs of the actions, in the order that the client runs them.
    pub actions: Vec<ObjectId>,
}

impl Event {
    /// Reads an event payload: the ID, a varint action count and the action IDs.
    pub(super) fn read(reader: &mut Reader<'_>) -> Step<Self> {
        let id = reader.id()?;
        let count = reader.var()?;
        let actions = reader.ids(count)?;
        Ok(Self { id, actions })
    }

    /// Writes an event payload.
    pub(super) fn write(&self, out: &mut Vec<u8>) -> Result<()> {
        out.put_u32(self.id.0);
        let count = super::to_u32(self.actions.len(), "action count")?;
        out.put_var(count);
        out.put_ids(&self.actions);
        Ok(())
    }
}
