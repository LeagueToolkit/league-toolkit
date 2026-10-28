//! The event query: the media that an event plays.

use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::{bank::BankVersion, Bank, ObjectId, Result, SourceId};

use super::{ActionType, Object, ObjectKind, StreamType, Typed};

/// One media that an event plays.
///
/// `stream_type` names the file that holds the media: a media index for
/// [`StreamType::DATA_BANK`], a package for [`StreamType::STREAMING`], both for
/// [`StreamType::PREFETCH`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventSource {
    /// The Sound that holds the source.
    pub sound: ObjectId,
    /// The ID of the media.
    pub source: SourceId,
    /// Where the media is stored.
    pub stream_type: StreamType,
}

/// Returns the media that the event `event` plays, from the hierarchies of `banks`.
///
/// The query finds an object by ID in the banks in order and takes the first match. It indexes
/// the objects of `banks` by ID once per call. It decodes each object with the version of the
/// bank that holds it. From the event it follows each action of the class Play to its target.
/// It reads the class of an action from bytes 4 to 6 of its payload and does not decode an
/// action of another class. A Sound gives one [`EventSource`]. A container gives the
/// sources of its children in order. Every other kind gives nothing. An object that no bank holds
/// gives nothing.
///
/// The query visits each object once. The result is in the order of the walk, depth first. An ID
/// whose first match is not an event gives an empty list.
///
/// ```
/// use ltk_wwise::{bank::{BankHeader, BankVersion}, hierarchy::event_sources, Bank, BankId, ObjectId};
///
/// let bank = Bank::new(BankHeader::new(BankVersion::V145, BankId(1)));
/// assert_eq!(event_sources(&[&bank], ObjectId(2))?, []);
/// # Ok::<(), ltk_wwise::Error>(())
/// ```
///
/// # Errors
///
/// The first error that decoding a visited object returns. An action of a class other than Play
/// is not decoded and returns no error.
///
/// - [`Error::UnsupportedLayout`](crate::Error::UnsupportedLayout) for a bank version other than
///   132, 134 and 145.
/// - [`Error::Layout`](crate::Error::Layout) when the layout of an object does not end at the end
///   of its payload.
/// - [`Error::ActionClass`](crate::Error::ActionClass) for an action class outside the table of
///   the generation.
pub fn event_sources(banks: &[&Bank], event: ObjectId) -> Result<Vec<EventSource>> {
    let mut sources = Vec::new();
    let objects = index(banks);
    let find = |id: ObjectId| objects.get(&id).copied();
    let Some((object, version)) = find(event) else {
        return Ok(sources);
    };
    let Some(Typed::Event(event)) = decode_as(object, version, Role::Event)? else {
        return Ok(sources);
    };
    let mut visited = HashSet::from([event.id]);
    let mut pending = Vec::new();
    push(&mut pending, &visited, &event.actions, Role::Action);
    while let Some((id, role)) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let Some((object, version)) = find(id) else {
            continue;
        };
        match decode_as(object, version, role)? {
            Some(Typed::Action(action)) if action.action_type.is_play() => {
                push(&mut pending, &visited, &[action.target], Role::Node);
            }
            Some(Typed::Sound(sound)) => sources.push(EventSource {
                sound: sound.id,
                source: sound.source.source_id,
                stream_type: sound.source.stream_type,
            }),
            Some(Typed::Container(container)) => {
                push(&mut pending, &visited, &container.children, Role::Node);
            }
            Some(Typed::Event(_) | Typed::Action(_)) | None => {}
        }
    }
    Ok(sources)
}

/// The place of an object in the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    /// The event of the query.
    Event,
    /// An action of the event.
    Action,
    /// The target of a Play action, or a child of a container.
    Node,
}

impl Role {
    /// Returns `true` when an object of the kind `kind` has a place in the walk in this role.
    fn fits(self, kind: ObjectKind) -> bool {
        match self {
            Self::Event => kind == ObjectKind::EVENT,
            Self::Action => kind == ObjectKind::ACTION,
            Self::Node => !matches!(kind, ObjectKind::EVENT | ObjectKind::ACTION),
        }
    }
}

/// Decodes `object` at `version` when its kind fits `role`, or returns `None`.
///
/// An action of a class other than Play is not decoded and gives `None`.
fn decode_as(object: &Object, version: BankVersion, role: Role) -> Result<Option<Typed>> {
    if !role.fits(object.kind()) {
        return Ok(None);
    }
    if role == Role::Action
        && ActionType::of_payload(object.payload()).is_some_and(|action| !action.is_play())
    {
        return Ok(None);
    }
    object.decode(version)
}

/// Pushes each ID of `ids` that the walk has not visited, last first.
///
/// The walk pops the IDs in the order of `ids`.
fn push(
    pending: &mut Vec<(ObjectId, Role)>,
    visited: &HashSet<ObjectId>,
    ids: &[ObjectId],
    role: Role,
) {
    let unvisited = ids.iter().rev().filter(|id| !visited.contains(id));
    pending.extend(unvisited.map(|&id| (id, role)));
}

/// Returns the objects of the hierarchies of `banks` by ID, each with the version of the bank
/// that holds it.
///
/// An ID maps to its first object in the first bank that holds it.
fn index<'a>(banks: &[&'a Bank]) -> HashMap<ObjectId, (&'a Object, BankVersion)> {
    let mut objects = HashMap::new();
    for bank in banks {
        let Some(hierarchy) = bank.hierarchy() else {
            continue;
        };
        let version = bank.header().version;
        for object in hierarchy.objects() {
            if let Entry::Vacant(slot) = objects.entry(object.id()) {
                slot.insert((object, version));
            }
        }
    }
    objects
}
