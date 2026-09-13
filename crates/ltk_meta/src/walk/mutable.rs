//! The mutable walk: the traversal of the read-only walk over an owned object through `&mut`.

use std::{
    fmt,
    ops::ControlFlow::{self, Break, Continue},
    ptr,
};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use super::{Interrupt, OwnedNode, Resume, Trail, TrailStep, TreeValue as _, Visit, WalkOutcome};
use crate::{property::values, Bin, BinObject, BinOverride, Error, PropertyValueEnum};

/// What a mutable walk calls.
///
/// The owned tree only. The traversal, the answers and the trail are the read-only walk's: a
/// `VisitorMut` that edits nothing sees the callbacks a [`Visitor`](super::Visitor) answering the
/// same sees, in the same order. Beside that, the tree is whatever the last callback left.
///
/// Every callback has a default that continues. A visitor implements only what it edits or reads.
#[expect(
    unused_variables,
    reason = "the defaults name their parameters for the reader and use none of them"
)]
pub trait VisitorMut {
    /// The visitor's own error. The crate's errors convert into it.
    type Error: From<Error>;

    /// Called at every node the walk reaches, before any of its properties.
    ///
    /// The walk walks the property map this callback leaves behind.
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn enter_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called once for every node entered: after its properties, after a [`Visit::Skip`], and
    /// while unwinding for a [`Visit::Stop`]. Never after an [`Visit::Abort`].
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn exit_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called for every property of a node, in property order, leaves included.
    ///
    /// The walk asks [`TreeValue::holds_node`](super::TreeValue::holds_node) of the value this
    /// callback leaves behind, and descends that value on [`Visit::Continue`].
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn enter_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called once for every property that holds a node and was entered: after its nodes, after
    /// a [`Visit::Skip`], and while unwinding for a [`Visit::Stop`]. Not called for a leaf. Never
    /// after an [`Visit::Abort`].
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn exit_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
}

/// A `&mut W` is a mutable visitor. A `&mut dyn VisitorMut<Error = E>` passes where one is
/// wanted.
impl<W: VisitorMut + ?Sized> VisitorMut for &mut W {
    type Error = W::Error;

    fn enter_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Self::Error> {
        (**self).enter_node(node)
    }

    fn exit_node(&mut self, node: &mut NodeMut<'_>) -> Result<Visit, Self::Error> {
        (**self).exit_node(node)
    }

    fn enter_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Self::Error> {
        (**self).enter_property(property)
    }

    fn exit_property(&mut self, property: &mut PropertyMut<'_>) -> Result<Visit, Self::Error> {
        (**self).exit_property(property)
    }
}

/// One node of a mutable walk: where it is, and its property map.
///
/// The class hash is read-only. A node inside a container, optional or map keeps the kind its
/// holder declares.
pub struct NodeMut<'t> {
    object_hash: BinHash,
    class_hash: BinHash,
    properties: &'t mut IndexMap<BinHash, PropertyValueEnum>,
    trail: &'t Trail<&'t PropertyValueEnum>,
}

impl<'t> NodeMut<'t> {
    /// The path hash of the object this node is in, or is.
    #[must_use]
    pub fn object_hash(&self) -> BinHash {
        self.object_hash
    }

    /// The class hash this node carries. Never 0 below the root.
    #[must_use]
    pub fn class_hash(&self) -> BinHash {
        self.class_hash
    }

    /// Where the node is: empty at the root. A map key in it is borrowed for the callback.
    #[must_use]
    pub fn trail(&self) -> &'t Trail<&'t PropertyValueEnum> {
        self.trail
    }

    /// Whether this node is the object itself.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.trail.is_empty()
    }

    /// The node read-only, as the read-only walk sees it: lookup by field, and
    /// [`TreeNode::to_struct`](super::TreeNode::to_struct).
    #[must_use]
    pub fn inner(&self) -> OwnedNode<'_> {
        OwnedNode::new(self.class_hash, self.properties)
    }

    /// The node's properties, in property order.
    #[must_use]
    pub fn properties(&self) -> &IndexMap<BinHash, PropertyValueEnum> {
        self.properties
    }

    /// The node's properties, to insert, remove, reorder or edit.
    #[must_use]
    pub fn properties_mut(&mut self) -> &mut IndexMap<BinHash, PropertyValueEnum> {
        self.properties
    }
}

impl fmt::Debug for NodeMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeMut")
            .field("object_hash", &self.object_hash)
            .field("class_hash", &self.class_hash)
            .field("property_count", &self.properties.len())
            .field("trail", &format_args!("{}", self.trail))
            .finish()
    }
}

/// One property of a mutable walk: where it is, and its value.
///
/// A property of a node carries no kind pin: the value can be replaced by a value of any kind.
pub struct PropertyMut<'t> {
    object_hash: BinHash,
    node_class_hash: BinHash,
    field: BinHash,
    value: &'t mut PropertyValueEnum,
    trail: &'t Trail<&'t PropertyValueEnum>,
}

impl<'t> PropertyMut<'t> {
    /// The path hash of the object the property is in.
    #[must_use]
    pub fn object_hash(&self) -> BinHash {
        self.object_hash
    }

    /// The class hash of the node the property is on. Never 0 below the root.
    #[must_use]
    pub fn node_class_hash(&self) -> BinHash {
        self.node_class_hash
    }

    /// The property's field hash.
    #[must_use]
    pub fn field(&self) -> BinHash {
        self.field
    }

    /// Where the node the property is on is: empty at the root. A map key in it is borrowed for
    /// the callback.
    #[must_use]
    pub fn trail(&self) -> &'t Trail<&'t PropertyValueEnum> {
        self.trail
    }

    /// The value.
    #[must_use]
    pub fn value(&self) -> &PropertyValueEnum {
        self.value
    }

    /// The value, to edit, or to replace with a value of any kind.
    #[must_use]
    pub fn value_mut(&mut self) -> &mut PropertyValueEnum {
        self.value
    }
}

impl fmt::Debug for PropertyMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PropertyMut")
            .field("object_hash", &self.object_hash)
            .field("node_class_hash", &self.node_class_hash)
            .field("field", &self.field)
            .field("kind", &self.value.kind())
            .field("trail", &format_args!("{}", self.trail))
            .finish()
    }
}

/// One mutable walk: the object hash, and the trail below the object's root.
///
/// `'w` is the borrow of the objects the walker walks. A key in the trail is typed for `'w` and is
/// valid only while it is on the trail.
struct WalkerMut<'w> {
    object_hash: BinHash,
    trail: Trail<&'w PropertyValueEnum>,
}

impl<'w> WalkerMut<'w> {
    fn new() -> Self {
        Self {
            // A placeholder: `walk_object` sets the hash before any callback reads it.
            object_hash: BinHash(0),
            trail: Trail::new(),
        }
    }

    /// Walks one object, and reports how it ended. The trail is empty afterwards, however the
    /// walk ended.
    fn walk_object<W: VisitorMut>(
        &mut self,
        object: &'w mut BinObject,
        visitor: &mut W,
    ) -> Result<WalkOutcome, W::Error> {
        self.object_hash = object.path_hash;
        self.trail.clear();
        Ok(
            match self.walk_node(object.class_hash, &mut object.properties, visitor)? {
                Continue(_) => WalkOutcome::Completed,
                Break(Interrupt::Unwind) => WalkOutcome::Stopped,
                Break(Interrupt::Abort) => WalkOutcome::Aborted,
            },
        )
    }

    fn node<'t>(
        &'t self,
        class_hash: BinHash,
        properties: &'t mut IndexMap<BinHash, PropertyValueEnum>,
    ) -> NodeMut<'t> {
        NodeMut {
            object_hash: self.object_hash,
            class_hash,
            properties,
            trail: &self.trail,
        }
    }

    fn property<'t>(
        &'t self,
        node_class_hash: BinHash,
        field: BinHash,
        value: &'t mut PropertyValueEnum,
    ) -> PropertyMut<'t> {
        PropertyMut {
            object_hash: self.object_hash,
            node_class_hash,
            field,
            value,
            trail: &self.trail,
        }
    }

    fn walk_node<W: VisitorMut>(
        &mut self,
        class_hash: BinHash,
        properties: &mut IndexMap<BinHash, PropertyValueEnum>,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt, Resume>, W::Error> {
        let walked = match visitor.enter_node(&mut self.node(class_hash, properties))? {
            Visit::Abort => return Ok(Break(Interrupt::Abort)),
            Visit::Stop => Break(Interrupt::Unwind),
            Visit::Skip => Continue(()),
            Visit::Continue => self.walk_properties(class_hash, properties, visitor)?,
        };
        if let Break(Interrupt::Abort) = walked {
            return Ok(Break(Interrupt::Abort));
        }

        Ok(
            match (
                walked,
                visitor.exit_node(&mut self.node(class_hash, properties))?,
            ) {
                (_, Visit::Abort) => Break(Interrupt::Abort),
                (Break(Interrupt::Unwind), _) | (_, Visit::Stop) => Break(Interrupt::Unwind),
                (_, Visit::Skip) => Continue(Resume::Parent),
                (_, Visit::Continue) => Continue(Resume::Siblings),
            },
        )
    }

    fn walk_properties<W: VisitorMut>(
        &mut self,
        class_hash: BinHash,
        properties: &mut IndexMap<BinHash, PropertyValueEnum>,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt>, W::Error> {
        for (&field, value) in properties.iter_mut() {
            let visit = visitor.enter_property(&mut self.property(class_hash, field, value))?;
            if !(&*value).holds_node()? {
                match visit {
                    Visit::Abort => return Ok(Break(Interrupt::Abort)),
                    Visit::Stop => return Ok(Break(Interrupt::Unwind)),
                    Visit::Skip | Visit::Continue => continue,
                }
            }

            let walked = match visit {
                Visit::Abort => return Ok(Break(Interrupt::Abort)),
                Visit::Stop => Break(Interrupt::Unwind),
                Visit::Skip => Continue(()),
                Visit::Continue => {
                    self.trail.push_field(field, class_hash);
                    let walked = self.descend(value, visitor);
                    self.trail.pop();
                    walked?
                }
            };
            if let Break(Interrupt::Abort) = walked {
                return Ok(Break(Interrupt::Abort));
            }

            match (
                walked,
                visitor.exit_property(&mut self.property(class_hash, field, value))?,
            ) {
                (_, Visit::Abort) => return Ok(Break(Interrupt::Abort)),
                (Break(Interrupt::Unwind), _) | (_, Visit::Stop) => {
                    return Ok(Break(Interrupt::Unwind))
                }
                (_, Visit::Skip) => break,
                (_, Visit::Continue) => {}
            }
        }
        Ok(Continue(()))
    }

    /// Descends a value that holds a node: the node itself, or every item of a container,
    /// optional or map.
    fn descend<W: VisitorMut>(
        &mut self,
        value: &mut PropertyValueEnum,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt>, W::Error> {
        match value {
            PropertyValueEnum::Struct(_) | PropertyValueEnum::Embedded(_) => {
                let Some(node) = as_node_mut(value) else {
                    return Ok(Continue(()));
                };
                Ok(
                    match self.walk_node(node.class_hash, &mut node.properties, visitor)? {
                        Break(interrupt) => Break(interrupt),
                        Continue(_) => Continue(()),
                    },
                )
            }
            PropertyValueEnum::Container(items) => self.descend_items(items.items_mut(), visitor),
            PropertyValueEnum::UnorderedContainer(items) => {
                self.descend_items(items.0.items_mut(), visitor)
            }
            PropertyValueEnum::Optional(optional) => match optional.slot() {
                Some(slot) => self.descend_items(std::iter::once(slot.into_inner()), visitor),
                None => Ok(Continue(())),
            },
            PropertyValueEnum::Map(map) => {
                for (key, value) in map.entries_mut() {
                    let Some(node) = as_node_mut(value) else {
                        continue;
                    };
                    // SAFETY: the reference is extended from the borrow of this entry to `'w`, the
                    // borrow of the object being walked, which outlives the entry. The invariant
                    // that makes it sound is the push and pop around `walk_node` below: the key is
                    // on the trail only between them, and the pop runs before the entry borrow ends
                    // and before the iterator reaches the next entry, error or not. While the key
                    // is on the trail nothing writes it: the walker holds the map through
                    // `entries_mut`, and a callback reaches only nodes inside this entry's value,
                    // which is disjoint from the key. A callback sees the trail under a borrow of
                    // its own length and cannot keep a key past it. A panic between push and pop
                    // leaves the key in a trail that is dropped and never read.
                    let key: &'w PropertyValueEnum = unsafe { &*ptr::from_ref(key) };
                    self.trail.push(TrailStep::Key(key));
                    let walked = self.walk_node(node.class_hash, &mut node.properties, visitor);
                    self.trail.pop();
                    match walked? {
                        Break(interrupt) => return Ok(Break(interrupt)),
                        Continue(Resume::Parent) => break,
                        Continue(Resume::Siblings) => {}
                    }
                }
                Ok(Continue(()))
            }
            _ => Ok(Continue(())),
        }
    }

    /// Descends the nodes among `items`, stepping into each by its index.
    fn descend_items<'i, W: VisitorMut>(
        &mut self,
        items: impl Iterator<Item = &'i mut PropertyValueEnum>,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt>, W::Error> {
        for (index, item) in items.enumerate() {
            let Some(node) = as_node_mut(item) else {
                continue;
            };
            self.trail.push(TrailStep::Index(index));
            let walked = self.walk_node(node.class_hash, &mut node.properties, visitor);
            self.trail.pop();
            match walked? {
                Break(interrupt) => return Ok(Break(interrupt)),
                Continue(Resume::Parent) => break,
                Continue(Resume::Siblings) => {}
            }
        }
        Ok(Continue(()))
    }
}

/// `value` as a node, if it is a `Struct` or `Embedded` with a class hash that is not 0.
fn as_node_mut(value: &mut PropertyValueEnum) -> Option<&mut values::Struct> {
    let node = match value {
        PropertyValueEnum::Struct(node) => node,
        PropertyValueEnum::Embedded(embedded) => &mut embedded.0,
        _ => return None,
    };
    (*node.class_hash != 0).then_some(node)
}

/// Walks `objects` in order through one walker. A `Stop` or `Abort` ends the whole walk.
fn walk_all_mut<'w, W: VisitorMut>(
    objects: impl IntoIterator<Item = &'w mut BinObject>,
    visitor: &mut W,
) -> Result<WalkOutcome, W::Error> {
    let mut walker = WalkerMut::new();
    for object in objects {
        match walker.walk_object(object, visitor)? {
            WalkOutcome::Completed => {}
            ended => return Ok(ended),
        }
    }
    Ok(WalkOutcome::Completed)
}

impl BinObject {
    /// Walks this object mutably, with the tree as each callback leaves it.
    ///
    /// The root, then every node beneath every property `visitor` enters.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk_mut<W: VisitorMut>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error> {
        WalkerMut::new().walk_object(self, visitor)
    }
}

impl Bin {
    /// Walks every object mutably, in file order. A `Stop` or `Abort` ends the whole walk, not
    /// the current object.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk_mut<W: VisitorMut>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error> {
        walk_all_mut(self.objects.values_mut(), visitor)
    }
}

impl BinOverride {
    /// Walks every embedded object mutably, in file order. Patch records are not walked.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk_mut<W: VisitorMut>(&mut self, visitor: &mut W) -> Result<WalkOutcome, W::Error> {
        walk_all_mut(self.objects.values_mut(), visitor)
    }
}
