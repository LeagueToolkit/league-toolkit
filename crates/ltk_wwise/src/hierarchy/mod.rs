//! The `HIRC` hierarchy: objects, typed layouts and the event query.
use std::{
    fmt,
    io::{self, Write},
    mem,
};

use byteorder::{WriteBytesExt, LE};

use crate::{bank::write_chunk_header, Error, ObjectId, Result};

mod action;
mod container;
mod event;
mod layout;
mod node;
mod sound;
mod sources;
mod typed;

pub use action::{Action, ActionType};
pub use container::Container;
pub use event::Event;
pub use node::NodeBase;
pub use sound::{BankSource, Sound, StreamType};
pub use sources::{event_sources, EventSource};
pub use typed::Typed;

/// The tag of the hierarchy chunk.
pub(crate) const HIRC: [u8; 4] = *b"HIRC";

/// The length of the `u32` object count at the start of a `HIRC` body.
const COUNT_LEN: usize = 4;

/// The length of the framing of one object: the `u8` kind and the `u32` size.
const OBJECT_HEADER_LEN: usize = 5;

/// The length of an object ID, the first field of every payload.
const ID_LEN: usize = 4;

/// The objects of a `HIRC` chunk, in file order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Hierarchy {
    objects: Vec<Object>,
}

impl Hierarchy {
    /// Returns a hierarchy with no objects.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of objects.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns `true` when the hierarchy holds no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Returns the objects, in file order.
    pub fn objects(&self) -> &[Object] {
        &self.objects
    }

    /// Returns the first object with the ID `id`.
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.iter().find(|object| object.id() == id)
    }

    /// Returns the first object with the ID `id` for editing.
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.iter_mut().find(|object| object.id() == id)
    }

    /// Returns the objects of the kind `kind`, in file order.
    pub fn of_kind(&self, kind: ObjectKind) -> impl Iterator<Item = &Object> {
        self.objects
            .iter()
            .filter(move |object| object.kind == kind)
    }

    /// Appends `object` after the last object.
    pub fn push(&mut self, object: Object) {
        self.objects.push(object);
    }

    /// Replaces the first object with the ID of `object` in place, or appends `object`.
    ///
    /// Returns the replaced object, or `None` when no object has the ID.
    pub fn replace(&mut self, object: Object) -> Option<Object> {
        match self.get_mut(object.id()) {
            Some(slot) => Some(mem::replace(slot, object)),
            None => {
                self.objects.push(object);
                None
            }
        }
    }

    /// Removes the first object with the ID `id`. The other objects keep their order.
    pub fn remove(&mut self, id: ObjectId) -> Option<Object> {
        let index = self.objects.iter().position(|object| object.id() == id)?;
        Some(self.objects.remove(index))
    }

    /// Decodes the body of a `HIRC` chunk.
    pub(crate) fn decode(body: &[u8]) -> Result<Self> {
        let len = body.len();
        let framing = |offset: usize| Error::Framing { offset, len };
        let (count, mut rest) = body
            .split_first_chunk::<COUNT_LEN>()
            .ok_or(framing(COUNT_LEN))?;
        let count = u32::from_le_bytes(*count);

        let mut objects = Vec::new();
        for _ in 0..count {
            let offset = len - rest.len();
            let Some((&[kind, s0, s1, s2, s3], after)) =
                rest.split_first_chunk::<OBJECT_HEADER_LEN>()
            else {
                return Err(framing(offset + OBJECT_HEADER_LEN));
            };
            let size = u32::from_le_bytes([s0, s1, s2, s3]) as usize;
            let Some((payload, after)) = after.split_at_checked(size) else {
                return Err(framing((offset + OBJECT_HEADER_LEN).saturating_add(size)));
            };
            objects.push(Object::new(ObjectKind(kind), payload.to_vec())?);
            rest = after;
        }
        if !rest.is_empty() {
            return Err(framing(len - rest.len()));
        }
        Ok(Self { objects })
    }

    /// Returns the length of the encoded body.
    fn encoded_len(&self) -> usize {
        self.objects.iter().fold(COUNT_LEN, |len, object| {
            len + OBJECT_HEADER_LEN + object.payload.len()
        })
    }

    /// Writes the `HIRC` chunk.
    pub(crate) fn write(&self, writer: &mut impl Write) -> Result<()> {
        write_chunk_header(writer, HIRC, self.encoded_len())?;
        writer.write_u32::<LE>(to_u32(self.objects.len(), "object count")?)?;
        for object in &self.objects {
            writer.write_u8(object.kind.0)?;
            writer.write_u32::<LE>(to_u32(object.payload.len(), "object payload")?)?;
            writer.write_all(&object.payload)?;
        }
        Ok(())
    }
}

/// Returns `value` as a `u32` field, or an error that names the field `what`.
fn to_u32(value: usize, what: &str) -> io::Result<u32> {
    u32::try_from(value).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{what} of {value} is larger than u32::MAX"),
        )
    })
}

/// One hierarchy object: a kind and a payload whose first `u32` is the object ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Object {
    kind: ObjectKind,
    payload: Vec<u8>,
}

impl Object {
    /// Returns the object of the kind `kind` and the payload `payload`.
    ///
    /// # Errors
    ///
    /// - [`Error::PayloadTooShort`] when `payload` is shorter than 4 bytes, the length of an ID.
    pub fn new(kind: ObjectKind, payload: Vec<u8>) -> Result<Self> {
        if payload.len() < ID_LEN {
            return Err(Error::PayloadTooShort(payload.len()));
        }
        Ok(Self { kind, payload })
    }

    /// Returns the kind.
    pub fn kind(&self) -> ObjectKind {
        self.kind
    }

    /// Returns the object ID: the first `u32` of the payload.
    pub fn id(&self) -> ObjectId {
        let [b0, b1, b2, b3, ..] = self.payload[..] else {
            unreachable!(
                "Object::new keeps payloads of 4 bytes or more, found {}",
                self.payload.len()
            );
        };
        ObjectId(u32::from_le_bytes([b0, b1, b2, b3]))
    }

    /// Returns the payload, the object ID included.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Returns the payload, the object ID included.
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }
}

/// The kind byte of a hierarchy object.
///
/// The crate names kinds 1 to 14. Kinds 15 to 22 exist in the version 145 reader.
///
/// ```
/// use ltk_wwise::hierarchy::ObjectKind;
///
/// assert_eq!(ObjectKind::EVENT.to_string(), "EVENT");
/// assert_eq!(ObjectKind(17).to_string(), "kind(17)");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectKind(pub u8);

impl ObjectKind {
    /// A state: 1.
    pub const STATE: Self = Self(1);
    /// A sound: 2.
    pub const SOUND: Self = Self(2);
    /// An action: 3.
    pub const ACTION: Self = Self(3);
    /// An event: 4.
    pub const EVENT: Self = Self(4);
    /// A random or sequence container: 5.
    pub const RANDOM_SEQUENCE_CONTAINER: Self = Self(5);
    /// A switch container: 6.
    pub const SWITCH_CONTAINER: Self = Self(6);
    /// An actor-mixer: 7.
    pub const ACTOR_MIXER: Self = Self(7);
    /// A bus: 8.
    pub const BUS: Self = Self(8);
    /// A layer container: 9.
    pub const LAYER_CONTAINER: Self = Self(9);
    /// A music segment: 10.
    pub const MUSIC_SEGMENT: Self = Self(10);
    /// A music track: 11.
    pub const MUSIC_TRACK: Self = Self(11);
    /// A music switch container: 12.
    pub const MUSIC_SWITCH_CONTAINER: Self = Self(12);
    /// A music random or sequence container: 13.
    pub const MUSIC_RANDOM_SEQUENCE_CONTAINER: Self = Self(13);
    /// An attenuation: 14.
    pub const ATTENUATION: Self = Self(14);

    /// Returns the name of the constant of the kind, or `None` for an unnamed kind.
    fn name(self) -> Option<&'static str> {
        Some(match self {
            Self::STATE => "STATE",
            Self::SOUND => "SOUND",
            Self::ACTION => "ACTION",
            Self::EVENT => "EVENT",
            Self::RANDOM_SEQUENCE_CONTAINER => "RANDOM_SEQUENCE_CONTAINER",
            Self::SWITCH_CONTAINER => "SWITCH_CONTAINER",
            Self::ACTOR_MIXER => "ACTOR_MIXER",
            Self::BUS => "BUS",
            Self::LAYER_CONTAINER => "LAYER_CONTAINER",
            Self::MUSIC_SEGMENT => "MUSIC_SEGMENT",
            Self::MUSIC_TRACK => "MUSIC_TRACK",
            Self::MUSIC_SWITCH_CONTAINER => "MUSIC_SWITCH_CONTAINER",
            Self::MUSIC_RANDOM_SEQUENCE_CONTAINER => "MUSIC_RANDOM_SEQUENCE_CONTAINER",
            Self::ATTENUATION => "ATTENUATION",
            _ => return None,
        })
    }
}

/// Writes the name of the constant, or `kind(N)` for a kind that the crate does not name.
impl fmt::Display for ObjectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => f.write_str(name),
            None => write!(f, "kind({})", self.0),
        }
    }
}
