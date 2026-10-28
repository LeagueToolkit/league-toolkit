//! The event query: the media that an event plays.

use std::collections::HashSet;

use crate::{bank::BankVersion, Bank, ObjectId, Result, SourceId};

use super::{Object, ObjectKind, StreamType, Typed};

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
/// The query finds an object by ID in the banks in order and takes the first match. It decodes
/// each object with the version of the bank that holds it. From the event it follows each action
/// of the class Play to its target. A Sound gives one [`EventSource`]. A container gives the
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
/// The first error that decoding a visited object returns:
///
/// - [`Error::UnsupportedLayout`](crate::Error::UnsupportedLayout) for a bank version other than
///   132, 134 and 145.
/// - [`Error::Layout`](crate::Error::Layout) when the layout of an object does not end at the end
///   of its payload.
/// - [`Error::ActionClass`](crate::Error::ActionClass) for an action class outside the table of
///   the generation.
pub fn event_sources(banks: &[&Bank], event: ObjectId) -> Result<Vec<EventSource>> {
    let mut sources = Vec::new();
    let Some((object, version)) = find(banks, event) else {
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
        let Some((object, version)) = find(banks, id) else {
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
fn decode_as(object: &Object, version: BankVersion, role: Role) -> Result<Option<Typed>> {
    if !role.fits(object.kind()) {
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

/// Returns the first object with the ID `id` in the hierarchies of `banks`, and the version of
/// the bank that holds it.
fn find<'a>(banks: &[&'a Bank], id: ObjectId) -> Option<(&'a Object, BankVersion)> {
    banks.iter().find_map(|bank| {
        let object = bank.hierarchy()?.get(id)?;
        Some((object, bank.header().version))
    })
}
