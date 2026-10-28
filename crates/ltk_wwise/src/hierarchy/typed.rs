//! Decoding and encoding of the typed hierarchy objects.

use crate::{bank::BankVersion, Error, ObjectId, Result};

use super::{
    layout::{Generation, Reader, Step},
    Action, Container, Event, Object, ObjectKind, Sound,
};

/// A decoded hierarchy object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Typed {
    Event(Event),
    Action(Action),
    Sound(Sound),
    Container(Container),
}

impl Typed {
    /// Returns the object ID.
    pub fn id(&self) -> ObjectId {
        match self {
            Self::Event(event) => event.id,
            Self::Action(action) => action.id,
            Self::Sound(sound) => sound.id,
            Self::Container(container) => container.id,
        }
    }

    /// Returns the kind of the encoded object.
    fn kind(&self) -> ObjectKind {
        match self {
            Self::Event(_) => ObjectKind::EVENT,
            Self::Action(_) => ObjectKind::ACTION,
            Self::Sound(_) => ObjectKind::SOUND,
            Self::Container(container) => container.kind,
        }
    }

    /// Encodes the object with the layouts of the generation of `version`.
    ///
    /// Encoding with the other generation writes zero into each field of the current generation,
    /// and drops it for the legacy generation. The encoded object decodes at `version`.
    ///
    /// # Errors
    ///
    /// - [`Error::UnsupportedLayout`] for a version other than
    ///   132, 134 and 145.
    /// - [`Error::ActionClass`] for an action class outside the
    ///   table of the generation.
    /// - [`Error::Layout`] when the fields kept as bytes do not fit the action class or the
    ///   container kind, at offset 0 for a container of a kind without a typed layout.
    pub fn encode(&self, version: BankVersion) -> Result<Object> {
        let generation = Generation::of(version)?;
        let mut payload = Vec::new();
        match self {
            Self::Event(event) => event.write(&mut payload)?,
            Self::Action(action) => action.write(&mut payload, generation),
            Self::Sound(sound) => sound.write(&mut payload, generation)?,
            Self::Container(container) => container.write(&mut payload, generation)?,
        }
        let object = Object::new(self.kind(), payload)?;
        match object.decode(version)? {
            Some(_) => Ok(object),
            None => Err(Error::Layout {
                kind: object.kind(),
                id: object.id(),
                offset: 0,
                len: object.payload().len(),
            }),
        }
    }
}

impl Object {
    /// Decodes the object with the layouts of the generation of `version`.
    ///
    /// Returns `None` for a kind without a typed layout. A typed layout decodes only when it
    /// consumes the whole payload.
    ///
    /// # Errors
    ///
    /// - [`Error::UnsupportedLayout`] for a version other than
    ///   132, 134 and 145.
    /// - [`Error::Layout`] when the layout of the kind does not end at the
    ///   end of the payload.
    /// - [`Error::ActionClass`] for an action class outside the
    ///   table of the generation.
    pub fn decode(&self, version: BankVersion) -> Result<Option<Typed>> {
        let generation = Generation::of(version)?;
        let mut reader = Reader::new(self.payload());
        let typed: Step<Typed> = match self.kind() {
            ObjectKind::EVENT => Event::read(&mut reader).map(Typed::Event),
            ObjectKind::ACTION => Action::read(&mut reader, generation).map(Typed::Action),
            ObjectKind::SOUND => Sound::read(&mut reader, generation).map(Typed::Sound),
            kind if Container::is_container(kind) => {
                Container::read(&mut reader, kind, generation).map(Typed::Container)
            }
            _ => return Ok(None),
        };
        typed
            .and_then(|typed| reader.finish().map(|()| typed))
            .map(Some)
            .map_err(|stop| stop.into_error(self.kind(), self.id(), self.payload().len()))
    }
}
