//! The event queries: the tree of an event and the media that it plays.

use std::collections::{hash_map::Entry, HashMap, HashSet};

use crate::{bank::BankVersion, Bank, ObjectId, Result, SourceId};

use super::{Action, ActionType, Container, Object, ObjectKind, Sound, StreamType, Typed};

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

impl EventSource {
    /// Returns the source of `sound`.
    fn of(sound: &Sound) -> Self {
        Self {
            sound: sound.id,
            source: sound.source.source_id,
            stream_type: sound.source.stream_type,
        }
    }
}

/// The objects that an event reaches: its actions, and the nodes under each Play action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventTree {
    /// The ID of the event.
    pub id: ObjectId,
    /// The actions that decode, in the order of the event.
    pub actions: Vec<EventAction>,
}

/// One action of an [`EventTree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventAction {
    pub action: Action,
    /// The node tree of the target of a Play action.
    ///
    /// `None` for an action of another class, and for a target that no bank holds, that is an
    /// event or an action, or that the walk has visited.
    pub target: Option<EventNode>,
}

/// One node of an [`EventTree`]: the target of a Play action, or a child of a container.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventNode {
    /// A Sound: a leaf with one media.
    Sound(Sound),
    /// A container and the trees of the children that the walk visits, in child order.
    Container {
        container: Container,
        children: Vec<EventNode>,
    },
    /// An object of a kind without a typed layout, such as a music object: a leaf.
    Other { id: ObjectId, kind: ObjectKind },
}

impl EventTree {
    /// Returns the Sounds of the tree as [`EventSource`]s, depth first.
    ///
    /// The sources are the leaves of the tree that are Sounds, in the order of
    /// [`event_sources`].
    pub fn sources(&self) -> impl Iterator<Item = EventSource> + '_ {
        let mut pending: Vec<&EventNode> = self
            .actions
            .iter()
            .rev()
            .filter_map(|action| action.target.as_ref())
            .collect();
        std::iter::from_fn(move || {
            while let Some(node) = pending.pop() {
                match node {
                    EventNode::Sound(sound) => return Some(EventSource::of(sound)),
                    EventNode::Container { children, .. } => pending.extend(children.iter().rev()),
                    EventNode::Other { .. } => {}
                }
            }
            None
        })
    }
}

/// Returns the tree of the event `event`, from the hierarchies of `banks`.
///
/// The query finds an object by ID in the banks in order and takes the first match. It indexes
/// the objects of `banks` by ID once per call. It decodes each object with the version of the
/// bank that holds it. The tree holds each action of the event that decodes. Under a Play action
/// it holds the target: a Sound is a leaf, a container holds the trees of its children in order,
/// and an object of a kind without a typed layout is a leaf of its kind. An object that no bank
/// holds, and an event or an action in the place of a node, gives no node.
///
/// The query visits each object once, depth first. A node that the walk has visited gives no
/// node again. An ID whose first match is not an event gives `None`.
///
/// ```
/// use ltk_wwise::{bank::{BankHeader, BankVersion}, hierarchy::event_tree, Bank, BankId, ObjectId};
///
/// let bank = Bank::new(BankHeader::new(BankVersion::V145, BankId(1)));
/// assert_eq!(event_tree(&[&bank], ObjectId(2))?, None);
/// # Ok::<(), ltk_wwise::Error>(())
/// ```
///
/// # Errors
///
/// The first error that decoding the event, a Play action or a node returns. An action of a
/// class other than Play that does not decode gives no action and returns no error.
///
/// - [`Error::UnsupportedLayout`](crate::Error::UnsupportedLayout) for a bank version other than
///   132, 134 and 145.
/// - [`Error::Layout`](crate::Error::Layout) when the layout of an object does not end at the end
///   of its payload.
/// - [`Error::ActionClass`](crate::Error::ActionClass) for an action class outside the table of
///   the generation.
pub fn event_tree(banks: &[&Bank], event: ObjectId) -> Result<Option<EventTree>> {
    let mut walk = Walk::new(banks);
    let Some((object, version)) = walk.find(event) else {
        return Ok(None);
    };
    if object.kind() != ObjectKind::EVENT {
        return Ok(None);
    }
    let Some(Typed::Event(event)) = object.decode(version)? else {
        return Ok(None);
    };
    walk.visited.insert(event.id);
    let mut actions = Vec::new();
    for &id in &event.actions {
        let Some(action) = walk.action(id)? else {
            continue;
        };
        let target = if action.action_type.is_play() {
            walk.node_tree(action.target)?
        } else {
            None
        };
        actions.push(EventAction { action, target });
    }
    Ok(Some(EventTree {
        id: event.id,
        actions,
    }))
}

/// Returns the media that the event `event` plays, from the hierarchies of `banks`.
///
/// The media are the leaves of [`event_tree`] that are Sounds, depth first:
/// [`EventTree::sources`]. An ID whose first match is not an event gives an empty list.
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
/// The errors of [`event_tree`].
pub fn event_sources(banks: &[&Bank], event: ObjectId) -> Result<Vec<EventSource>> {
    Ok(event_tree(banks, event)?
        .map(|tree| tree.sources().collect())
        .unwrap_or_default())
}

/// The state of one query: the objects by ID and the IDs that the walk has visited.
struct Walk<'a> {
    objects: HashMap<ObjectId, (&'a Object, BankVersion)>,
    visited: HashSet<ObjectId>,
}

/// What a visit of a node gives.
enum Visit {
    /// No node: a visited ID, an ID that no bank holds, or an event or an action.
    Nothing,
    /// A leaf node.
    Leaf(EventNode),
    /// A container, whose children the walk visits next.
    Container(Container),
}

/// A container whose children the walk is visiting.
struct Frame {
    container: Container,
    /// The index in `container.children` of the next child to visit.
    next: usize,
    children: Vec<EventNode>,
}

impl Frame {
    fn new(container: Container) -> Self {
        Self {
            container,
            next: 0,
            children: Vec::new(),
        }
    }

    /// Returns the next child to visit, or `None` after the last child.
    fn next_child(&mut self) -> Option<ObjectId> {
        let child = self.container.children.get(self.next).copied()?;
        self.next += 1;
        Some(child)
    }

    fn into_node(self) -> EventNode {
        EventNode::Container {
            container: self.container,
            children: self.children,
        }
    }
}

impl<'a> Walk<'a> {
    fn new(banks: &[&'a Bank]) -> Self {
        Self {
            objects: index(banks),
            visited: HashSet::new(),
        }
    }

    fn find(&self, id: ObjectId) -> Option<(&'a Object, BankVersion)> {
        self.objects.get(&id).copied()
    }

    /// Visits the action `id` and returns it decoded.
    ///
    /// An action of a class other than Play that does not decode gives `None`. The class comes
    /// from bytes 4 to 6 of the payload.
    fn action(&mut self, id: ObjectId) -> Result<Option<Action>> {
        if !self.visited.insert(id) {
            return Ok(None);
        }
        let Some((object, version)) = self.find(id) else {
            return Ok(None);
        };
        if object.kind() != ObjectKind::ACTION {
            return Ok(None);
        }
        let decoded = object.decode(version);
        let is_play = ActionType::of_payload(object.payload()).is_none_or(ActionType::is_play);
        match decoded {
            Ok(Some(Typed::Action(action))) => Ok(Some(action)),
            Err(error) if is_play => Err(error),
            _ => Ok(None),
        }
    }

    /// Visits the node `id`.
    fn node(&mut self, id: ObjectId) -> Result<Visit> {
        if !self.visited.insert(id) {
            return Ok(Visit::Nothing);
        }
        let Some((object, version)) = self.find(id) else {
            return Ok(Visit::Nothing);
        };
        let kind = object.kind();
        if matches!(kind, ObjectKind::EVENT | ObjectKind::ACTION) {
            return Ok(Visit::Nothing);
        }
        Ok(match object.decode(version)? {
            Some(Typed::Sound(sound)) => Visit::Leaf(EventNode::Sound(sound)),
            Some(Typed::Container(container)) => Visit::Container(container),
            Some(_) => Visit::Nothing,
            None => Visit::Leaf(EventNode::Other { id, kind }),
        })
    }

    /// Returns the tree of the node `root`, depth first, with a stack of the open containers.
    fn node_tree(&mut self, root: ObjectId) -> Result<Option<EventNode>> {
        let mut open: Vec<Frame> = Vec::new();
        let mut next = Some(root);
        loop {
            let mut done = match next.take() {
                Some(id) => match self.node(id)? {
                    Visit::Nothing => None,
                    Visit::Leaf(node) => Some(node),
                    Visit::Container(container) => {
                        open.push(Frame::new(container));
                        None
                    }
                },
                None => None,
            };
            loop {
                if let Some(node) = done.take() {
                    match open.last_mut() {
                        Some(parent) => parent.children.push(node),
                        None => return Ok(Some(node)),
                    }
                }
                let Some(frame) = open.last_mut() else {
                    return Ok(None);
                };
                if let Some(child) = frame.next_child() {
                    next = Some(child);
                    break;
                }
                done = open.pop().map(Frame::into_node);
            }
        }
    }
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
