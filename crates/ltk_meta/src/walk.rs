//! One read-only traversal over every node of a bin object, driven by a [`Visitor`].
//!
//! The walk is written once, against two sealed traits - [`TreeValue`] and [`TreeNode`] - that
//! the owned tree (`&PropertyValueEnum<M>`) and the streaming view ([`ValueView`]) both
//! implement. A visitor is generic over the value type and runs over either unchanged:
//! [`BinObject::walk`] and [`Bin::walk`] over the owned tree, [`ObjectView::walk`] and
//! [`BinStream::walk`] over a buffered object's bytes.
//!
//! The visitor sees nodes in pre-order, in file order, each exactly once. A node is an object,
//! or a `Struct` or `Embedded` value whose class hash is not 0. It is entered and exited, and
//! so is every property of it that can hold a node. Every callback answers a [`Visit`]; the
//! walk returns a [`WalkOutcome`], or the visitor's own error.
//!
//! The walk carries a [`Trail`]: the steps from the object's root to the current position,
//! borrowing the tree and allocating nothing per step. A visitor renders it for a node it
//! reports on and for nothing else.
//!
//! ```
//! use ltk_hash::BinHash;
//! use ltk_meta::{
//!     concrete::{values, Bin, BinObject},
//!     walk::{Node, TreeValue, Visit, Visitor},
//!     Error,
//! };
//!
//! /// Every node's address, in pre-order.
//! #[derive(Default)]
//! struct Addresses(Vec<String>);
//!
//! impl<'a, V: TreeValue<'a>> Visitor<'a, V> for Addresses {
//!     type Error = Error;
//!
//!     fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Error> {
//!         self.0.push(format!("{:08x} {}", node.object_hash(), node.trail()));
//!         Ok(Visit::Continue)
//!     }
//! }
//!
//! let inner = values::Struct {
//!     class_hash: 0xC1A5_0002u32.into(),
//!     properties: Default::default(),
//!     meta: Default::default(),
//! };
//! let bin = Bin::builder()
//!     .object(
//!         BinObject::builder(0x0100_0001u32, 0xC1A5_0001u32)
//!             .property(0x0000_0001u32, inner)
//!             .build(),
//!     )
//!     .build();
//!
//! let mut addresses = Addresses::default();
//! bin.walk(&mut addresses)?;
//! assert_eq!(addresses.0, ["01000001 ", "01000001 00000001"]);
//! # Ok::<(), Error>(())
//! ```

mod owned;
mod tree;
mod view;

#[cfg(test)]
mod tests;

pub use owned::{OwnedChildren, OwnedNode, OwnedProperties};
pub use tree::{Child, Leaf, TreeNode, TreeValue};
pub use view::{ViewChildren, ViewProperties};

use std::{
    fmt, io,
    ops::ControlFlow::{self, Break, Continue},
};

use ltk_hash::BinHash;

use crate::{
    stream::{BinStream, ObjectStream, ObjectView, ValueView},
    Bin, BinObject, BinOverride, Error, PropertyValueEnum,
};

/// What a callback answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Visit {
    /// Ends the walk immediately. No exit callback runs for anything open.
    Abort,
    /// Ends the walk after unwinding: every open property and node gets its exit,
    /// innermost first. The walk does not resume.
    Stop,
    /// Skips ahead, locally:
    /// - from [`Visitor::enter_node`]: the node's properties are not walked; its
    ///   [`Visitor::exit_node`] runs regardless.
    /// - from [`Visitor::enter_property`]: the value is not descended - the prune.
    ///   [`Visitor::exit_property`] runs regardless for a value that holds a node.
    /// - from [`Visitor::exit_property`]: the node's remaining properties are pruned; the walk
    ///   jumps to the node's [`Visitor::exit_node`].
    /// - from [`Visitor::exit_node`]: the parent property's remaining items are pruned; the
    ///   walk jumps to the parent's [`Visitor::exit_property`].
    Skip,
    /// Carries on.
    Continue,
}

/// How a walk ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkOutcome {
    /// Every object was walked to the end.
    Completed,
    /// A callback answered [`Visit::Stop`] and the walk unwound.
    Stopped,
    /// A callback answered [`Visit::Abort`].
    Aborted,
}

/// What a walk calls.
///
/// Generic over the tree's value type: one visitor runs over the owned tree
/// (`V = &PropertyValueEnum<M>`) and over the view (`V = ValueView<'a, M>`) alike.
///
/// Every callback has a default that continues. A visitor implements only what it reads.
#[expect(
    unused_variables,
    reason = "the defaults name their parameters for the reader and use none of them"
)]
pub trait Visitor<'a, V: TreeValue<'a>> {
    /// The visitor's own error. The tree's errors convert into it: a `?` on a tree call
    /// inside a callback needs nothing more than `From<ltk_meta::Error>`.
    type Error: From<Error>;

    /// Called at every node the walk reaches, before any of its properties.
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called once for every node entered: after its properties, after a [`Visit::Skip`],
    /// and while unwinding for a [`Visit::Stop`]. Never after an [`Visit::Abort`].
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn exit_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called for every property of a node, in file order, leaves included. The value is the
    /// tree's, undecoded until read. Only a value that [`TreeValue::holds_node`] is descended
    /// on [`Visit::Continue`]; a leaf is a call and nothing more.
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn enter_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }

    /// Called once for every property that holds a node and was entered: after its nodes,
    /// after a [`Visit::Skip`], and while unwinding for a [`Visit::Stop`]. Not called for a
    /// leaf. Never after an [`Visit::Abort`].
    ///
    /// # Errors
    ///
    /// The visitor's own. An error ends the walk at once, as an [`Visit::Abort`] does.
    fn exit_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Self::Error> {
        Ok(Visit::Continue)
    }
}

/// A `&mut W` is a visitor. A `&mut dyn Visitor<'a, V, Error = E>` passes where one is wanted.
impl<'a, V: TreeValue<'a>, W: Visitor<'a, V> + ?Sized> Visitor<'a, V> for &mut W {
    type Error = W::Error;

    fn enter_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        (**self).enter_node(node)
    }

    fn exit_node(&mut self, node: &Node<'_, 'a, V>) -> Result<Visit, Self::Error> {
        (**self).exit_node(node)
    }

    fn enter_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Self::Error> {
        (**self).enter_property(field, value, node)
    }

    fn exit_property(
        &mut self,
        field: BinHash,
        value: V,
        node: &Node<'_, 'a, V>,
    ) -> Result<Visit, Self::Error> {
        (**self).exit_property(field, value, node)
    }
}

/// One node, as the walk hands it to a visitor.
pub struct Node<'t, 'a, V: TreeValue<'a>> {
    object_hash: BinHash,
    inner: V::Node,
    trail: &'t Trail<V>,
}

impl<'t, 'a, V: TreeValue<'a>> Node<'t, 'a, V> {
    fn new(object_hash: BinHash, inner: V::Node, trail: &'t Trail<V>) -> Self {
        Self {
            object_hash,
            inner,
            trail,
        }
    }

    /// The path hash of the object this node is in, or is.
    #[must_use]
    pub fn object_hash(&self) -> BinHash {
        self.object_hash
    }

    /// The class hash this node carries. Never 0.
    #[must_use]
    pub fn class_hash(&self) -> BinHash {
        self.inner.class_hash()
    }

    /// The node itself: its properties in file order, lookup by field, and
    /// [`TreeNode::to_struct`].
    #[must_use]
    pub fn inner(&self) -> V::Node {
        self.inner
    }

    /// Where the node is: empty at the root.
    #[must_use]
    pub fn trail(&self) -> &'t Trail<V> {
        self.trail
    }

    /// Whether this node is the object itself.
    #[must_use]
    pub fn is_root(&self) -> bool {
        self.trail.is_empty()
    }
}

impl<'a, V: TreeValue<'a>> Clone for Node<'_, 'a, V> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, V: TreeValue<'a>> Copy for Node<'_, 'a, V> {}

impl<'a, V: TreeValue<'a>> fmt::Debug for Node<'_, 'a, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Node")
            .field("object_hash", &self.object_hash)
            .field("class_hash", &self.class_hash())
            .field("trail", &format_args!("{}", self.trail))
            .finish()
    }
}

/// One step of a [`Trail`]: a field, an index or a map entry. A key is the tree's value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrailStep<V> {
    /// A property of a node, by the field's name hash.
    Field(BinHash),
    /// A container element by position, or the value of a present optional, which is always 0.
    Index(usize),
    /// A map entry, by its key.
    Key(V),
}

/// The steps from an object's root to the walk's position.
///
/// Borrows the tree - a map key is the tree's own value, never a copy. Descending a map of ten
/// thousand entries allocates nothing. Text is made only by `Display`.
///
/// Beside the steps the trail keeps the **class context**: for each `Field` step, the class
/// hash of the node the field was read on. It is what a name table is asked with.
#[derive(Debug)]
pub struct Trail<V> {
    steps: Vec<TrailStep<V>>,
    classes: Vec<BinHash>,
}

impl<V> Trail<V> {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            classes: Vec::new(),
        }
    }

    /// The steps, root first.
    #[must_use]
    pub fn steps(&self) -> &[TrailStep<V>] {
        &self.steps
    }

    /// How many steps the trail holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the trail is at the root.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The class of the node each field step was read on, one per `Field` step, in order.
    /// Never 0: the walk always knows.
    #[must_use]
    pub fn classes(&self) -> &[BinHash] {
        &self.classes
    }

    fn push_field(&mut self, field: BinHash, class: BinHash) {
        self.steps.push(TrailStep::Field(field));
        self.classes.push(class);
    }

    fn push(&mut self, step: TrailStep<V>) {
        self.steps.push(step);
    }

    fn pop(&mut self) {
        if let Some(TrailStep::Field(_)) = self.steps.pop() {
            self.classes.pop();
        }
    }

    fn clear(&mut self) {
        self.steps.clear();
        self.classes.clear();
    }
}

/// The hash form: `.` between fields, `[i]` for an index, `{key}` for a map entry, every
/// field hash as eight lowercase hex digits. A key that does not decode renders as `{?}`.
impl<'a, V: TreeValue<'a>> fmt::Display for Trail<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, step) in self.steps.iter().enumerate() {
            match step {
                TrailStep::Field(field) => {
                    if i > 0 {
                        f.write_str(".")?;
                    }
                    write!(f, "{field:08x}")?;
                }
                TrailStep::Index(index) => write!(f, "[{index}]")?,
                TrailStep::Key(key) => {
                    f.write_str("{")?;
                    match key.leaf() {
                        Ok(Some(leaf)) => leaf.write_key(f)?,
                        Ok(None) | Err(_) => f.write_str("?")?,
                    }
                    f.write_str("}")?;
                }
            }
        }
        Ok(())
    }
}

/// Walk teardown, propagated up the recursion.
enum Interrupt {
    /// A [`Visit::Stop`]: every open exit runs, innermost first.
    Unwind,
    /// A [`Visit::Abort`], or an error: no further callback runs.
    Abort,
}

/// Where the walk resumes after a node's exit.
enum Resume {
    /// With the enclosing property's remaining items.
    Siblings,
    /// At the enclosing property's exit: the node's exit answered [`Visit::Skip`].
    Parent,
}

/// One object's walk: the object hash, and the trail below its root.
struct Walker<V> {
    object_hash: BinHash,
    trail: Trail<V>,
}

impl<'a, V: TreeValue<'a>> Walker<V> {
    fn new() -> Self {
        Self {
            // A placeholder: `walk_object` sets the hash before any callback reads it.
            object_hash: BinHash(0),
            trail: Trail::new(),
        }
    }

    /// Walks one object, `root` under `object_hash`, and reports how it ended. `Completed`
    /// leaves the trail empty and the walker ready for the next object.
    fn walk_object<W: Visitor<'a, V>>(
        &mut self,
        object_hash: BinHash,
        root: V::Node,
        visitor: &mut W,
    ) -> Result<WalkOutcome, W::Error> {
        self.object_hash = object_hash;
        self.trail.clear();
        Ok(match self.walk_node(root, visitor)? {
            Continue(_) => WalkOutcome::Completed,
            Break(Interrupt::Unwind) => WalkOutcome::Stopped,
            Break(Interrupt::Abort) => WalkOutcome::Aborted,
        })
    }

    fn node(&self, inner: V::Node) -> Node<'_, 'a, V> {
        Node::new(self.object_hash, inner, &self.trail)
    }

    fn walk_node<W: Visitor<'a, V>>(
        &mut self,
        node: V::Node,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt, Resume>, W::Error> {
        let walked = match visitor.enter_node(&self.node(node))? {
            Visit::Abort => return Ok(Break(Interrupt::Abort)),
            Visit::Stop => Break(Interrupt::Unwind),
            Visit::Skip => Continue(()),
            Visit::Continue => self.walk_properties(node, visitor)?,
        };
        if let Break(Interrupt::Abort) = walked {
            return Ok(Break(Interrupt::Abort));
        }

        Ok(match (walked, visitor.exit_node(&self.node(node))?) {
            (_, Visit::Abort) => Break(Interrupt::Abort),
            (Break(Interrupt::Unwind), _) | (_, Visit::Stop) => Break(Interrupt::Unwind),
            (_, Visit::Skip) => Continue(Resume::Parent),
            (_, Visit::Continue) => Continue(Resume::Siblings),
        })
    }

    fn walk_properties<W: Visitor<'a, V>>(
        &mut self,
        node: V::Node,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt>, W::Error> {
        for property in node.properties() {
            let (field, value) = property?;
            let visit = visitor.enter_property(field, value, &self.node(node))?;
            if !value.holds_node()? {
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
                    self.trail.push_field(field, node.class_hash());
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
                visitor.exit_property(field, value, &self.node(node))?,
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
    fn descend<W: Visitor<'a, V>>(
        &mut self,
        value: V,
        visitor: &mut W,
    ) -> Result<ControlFlow<Interrupt>, W::Error> {
        if let Some(node) = value.as_node()? {
            return Ok(match self.walk_node(node, visitor)? {
                Break(interrupt) => Break(interrupt),
                Continue(_) => Continue(()),
            });
        }

        for child in value.children()? {
            let (step, item) = child?;
            let Some(node) = item.as_node()? else {
                continue;
            };
            self.trail.push(match step {
                Child::Index(index) => TrailStep::Index(index),
                Child::Key(key) => TrailStep::Key(key),
            });
            let walked = self.walk_node(node, visitor);
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

/// Walks `objects` in order through one walker. A `Stop` or `Abort` ends the whole walk.
fn walk_all<'a, V, W, N>(
    objects: impl IntoIterator<Item = (BinHash, N)>,
    visitor: &mut W,
) -> Result<WalkOutcome, W::Error>
where
    V: TreeValue<'a, Node = N>,
    N: TreeNode<'a, Value = V>,
    W: Visitor<'a, V>,
{
    let mut walker = Walker::new();
    for (object_hash, root) in objects {
        match walker.walk_object(object_hash, root, visitor)? {
            WalkOutcome::Completed => {}
            ended => return Ok(ended),
        }
    }
    Ok(WalkOutcome::Completed)
}

impl<M> BinObject<M> {
    /// Walks this object: the root, then every node beneath every property `visitor` enters.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where
        W: Visitor<'a, &'a PropertyValueEnum<M>>,
    {
        Walker::new().walk_object(self.path_hash, OwnedNode::from(self), visitor)
    }
}

impl<M> Bin<M> {
    /// Walks every object, in file order. A `Stop` or `Abort` ends the whole walk, not the
    /// current object.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where
        W: Visitor<'a, &'a PropertyValueEnum<M>>,
    {
        walk_all(
            self.objects
                .values()
                .map(|object| (object.path_hash, OwnedNode::from(object))),
            visitor,
        )
    }
}

impl<M> BinOverride<M> {
    /// Walks every embedded object, in file order, as the file holds them. A record that
    /// targets one of them has not been applied. Patch records are not walked: a record's
    /// value has no node of its own to stand on.
    ///
    /// # Errors
    ///
    /// Whatever the visitor raises. The owned tree never fails on its own.
    pub fn walk<'a, W>(&'a self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where
        W: Visitor<'a, &'a PropertyValueEnum<M>>,
    {
        walk_all(
            self.objects
                .values()
                .map(|object| (object.path_hash, OwnedNode::from(object))),
            visitor,
        )
    }
}

impl<'a, M: Default> ObjectView<'a, M> {
    /// Walks this object over its buffered bytes: nothing is materialised, a header is
    /// decoded where the walk descends, and a leaf is decoded only when the visitor asks.
    ///
    /// # Errors
    ///
    /// A kind byte or header that does not decode, converted into the visitor's error, or
    /// whatever the visitor raises.
    pub fn walk<W>(&self, visitor: &mut W) -> Result<WalkOutcome, W::Error>
    where
        W: Visitor<'a, ValueView<'a, M>>,
    {
        Walker::new().walk_object(self.path_hash(), self.as_struct(), visitor)
    }
}

impl<R: io::Read + io::Seek, M: Default> ObjectStream<'_, R, M> {
    /// [`ObjectStream::view`] then [`ObjectView::walk`].
    ///
    /// `E` is the visitor's error, named once here: a visitor that runs over every object
    /// buffer is bound for every lifetime, and its error type is the one thing the bound
    /// holds fixed.
    ///
    /// # Errors
    ///
    /// The same as [`ObjectStream::view`] and [`ObjectView::walk`], in the visitor's error.
    pub fn walk<E, W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, E>
    where
        E: From<Error>,
        W: for<'a> Visitor<'a, ValueView<'a, M>, Error = E>,
    {
        self.view()?.walk(visitor)
    }
}

impl<R: io::Read + io::Seek, M: Default> BinStream<R, M> {
    /// Walks every object in file order, one buffered object at a time: [`BinStream::objects`]
    /// and [`ObjectStream::walk`] on each. Holds one object's bytes at any moment and nothing
    /// of the tree.
    ///
    /// # Errors
    ///
    /// The same as [`ObjectStream::walk`], for the object the walk was in.
    pub fn walk<E, W>(&mut self, visitor: &mut W) -> Result<WalkOutcome, E>
    where
        E: From<Error>,
        W: for<'a> Visitor<'a, ValueView<'a, M>, Error = E>,
    {
        let mut objects = self.objects();
        while let Some(mut object) = objects.next()? {
            match object.walk(visitor)? {
                WalkOutcome::Completed => {}
                ended => return Ok(ended),
            }
        }
        Ok(WalkOutcome::Completed)
    }
}
