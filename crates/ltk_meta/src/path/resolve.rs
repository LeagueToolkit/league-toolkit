//! Walking a [`PropertyPath`] through a value tree, and the type rule that governs a patch.

use std::fmt;

use indexmap::IndexMap;
use ltk_hash::{BinHash, Hash as _, WadHash};

use crate::{
    path::{KeyLiteral, PropertyPath, Segment, Subscript},
    property::{values, Kind},
    Bin, BinObject, PropertyValueEnum, ValueSlot,
};

/// What the patch type rule compares: the kind, a container's item and key kinds, and the class
/// of an embed.
///
/// The client compares a record's kind byte against the property's registered type, and then, in
/// the reader the record shares with an ordinary property, the element tags of a container, both
/// tags of a map, and the exact class of an embed. A pointer is the exception: the client accepts
/// any class that derives from the declared one. Deciding that needs the class hierarchy, which
/// only the game has, so a pointer's class is left out of the comparison entirely and this crate
/// accepts a pointer the client might still reject.
///
/// # Examples
///
/// ```
/// use ltk_meta::{path::ValueShape, property::values, PropertyValueEnum};
///
/// let anchor: PropertyValueEnum = values::Vector2::default().into();
/// let normal: PropertyValueEnum = values::Vector3::default().into();
///
/// assert_eq!(ValueShape::of(&anchor).to_string(), "Vector2");
/// assert!(ValueShape::of(&anchor).matches(&ValueShape::of(&anchor)));
/// assert!(!ValueShape::of(&anchor).matches(&ValueShape::of(&normal)));
///
/// // A container also compares by what it holds.
/// let links: PropertyValueEnum = values::Container::from(vec![values::ObjectLink::default()]).into();
/// assert_eq!(ValueShape::of(&links).to_string(), "Container[ObjectLink]");
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueShape {
    /// The kind of the value.
    pub kind: Kind,
    /// For a container or an option the item kind, for a map the value kind.
    pub item_kind: Option<Kind>,
    /// For a map the key kind.
    pub key_kind: Option<Kind>,
    /// For an embed the class it is an instance of. A pointer's class is deliberately not
    /// recorded; see the type note above.
    pub class: Option<BinHash>,
}

impl ValueShape {
    /// The shape of `value`.
    #[must_use]
    pub fn of<M>(value: &PropertyValueEnum<M>) -> Self {
        use PropertyValueEnum as V;

        let (item_kind, key_kind, class) = match value {
            V::Container(list) => (Some(list.item_kind()), None, None),
            V::UnorderedContainer(values::UnorderedContainer(list)) => {
                (Some(list.item_kind()), None, None)
            }
            V::Optional(option) => (Some(option.item_kind()), None, None),
            V::Map(map) => (Some(map.value_kind()), Some(map.key_kind()), None),
            V::Embedded(values::Embedded(embed)) => (None, None, Some(embed.class_hash)),
            _ => (None, None, None),
        };

        Self {
            kind: value.kind(),
            item_kind,
            key_kind,
            class,
        }
    }

    /// Whether the type rule accepts a value of shape `other` where this shape is expected.
    ///
    /// Every field is compared, so this currently agrees with `==`. It is a separate method
    /// because it names a rule rather than structural equality: an implementation that learned
    /// the class hierarchy could accept a pointer to a derived class here without changing what
    /// two shapes being equal means.
    #[must_use]
    #[inline]
    pub fn matches(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Display for ValueShape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.kind)?;
        match (self.key_kind, self.item_kind) {
            (Some(key), Some(item)) => write!(f, "[{key:?}, {item:?}]")?,
            (None, Some(item)) => write!(f, "[{item:?}]")?,
            _ => {}
        }
        match self.class {
            Some(class) => write!(f, " {class:08x}"),
            None => Ok(()),
        }
    }
}

/// Why a [`PropertyPath`] does not name a value in the tree it was walked through.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{kind} (segment {segment})")]
pub struct ResolveError {
    segment: usize,
    kind: ResolveErrorKind,
}

impl ResolveError {
    pub(crate) fn new(segment: usize, kind: ResolveErrorKind) -> Self {
        Self { segment, kind }
    }

    /// Which segment could not be applied, counting from 0.
    ///
    /// A segment that cannot be applied because the value before it is a leaf is charged to the
    /// segment, not to the leaf: `Enabled.Size` where `Enabled` is a bool fails at segment 1.
    /// [`ResolveErrorKind::MissingObject`] happens before any segment and reports 0.
    #[must_use]
    #[inline]
    pub fn segment(&self) -> usize {
        self.segment
    }

    /// What went wrong.
    #[must_use]
    #[inline]
    pub fn kind(&self) -> ResolveErrorKind {
        self.kind
    }
}

/// The kinds of [`ResolveError`].
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ResolveErrorKind {
    /// The bin has no object with this path hash. Only [`Bin::resolve`] produces this.
    #[error("no object {0:08x}")]
    MissingObject(BinHash),
    /// The value being descended into has no property with this name hash.
    #[error("no property {0:08x}")]
    MissingProperty(BinHash),
    /// A pointer with a class hash of 0.
    ///
    /// The client dereferences unconditionally and does not document what a null pointee does, so
    /// stopping here is this crate's choice rather than a reproduction of the client's.
    #[error("the pointer is null")]
    NullPointer,
    /// A `.name` piece applied to something that is not a pointer or an embed.
    #[error("cannot descend into a {0:?}")]
    CannotDescend(Kind),
    /// A `[i]` applied to something that is not a list, list2 or option, or a `{k}` applied to
    /// something that is not a map.
    #[error("a {0:?} cannot be subscripted that way")]
    NotIndexable(Kind),
    /// A `[i]` past the end of a list, or anything but `[0]` on a present option.
    #[error("index {index} is out of range, the length is {len}")]
    IndexOutOfRange {
        /// The index that was asked for.
        index: u32,
        /// How many elements there were. An option counts as 0 or 1.
        len: usize,
    },
    /// A `{k}` whose literal does not convert to the map's key kind.
    #[error("the key does not convert to {0:?}")]
    InvalidKey(Kind),
    /// A `{k}` that converts but matches no entry.
    #[error("no entry has that key")]
    KeyNotFound,
}

/// Why a patch record does not apply.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PatchError {
    /// The path does not name a value in the object.
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    /// The path names a value of a different type. Nothing was changed.
    #[error("type mismatch: the property is {expected}, the patch carries {found}")]
    TypeMismatch {
        /// The shape of the value already in the tree.
        expected: ValueShape,
        /// The shape of the value the patch carries.
        found: ValueShape,
    },
}

/// Which slot inside a value a subscript selects.
///
/// Decided on a shared borrow so the rules, and every error they can raise, are written once and
/// the two projections that follow cannot fail.
#[derive(Debug, Clone, Copy)]
enum Slot {
    /// An item of a list or a list2.
    Item(usize),
    /// The value inside an option, which is present.
    OptionValue,
    /// The value of a map entry.
    Entry(usize),
}

const MISSING: &str = "a Slot only ever names a slot that exists";

/// The properties of an object, a pointer's target or an embed: what a `.name` piece looks in.
type Properties<M> = IndexMap<BinHash, PropertyValueEnum<M>>;

/// Where the last segment of a path lands: the properties it is looked up in, the segment itself
/// and its position, for the error if it does not resolve.
type Landing<'a, 'p, M> = (&'a Properties<M>, Segment<'p>, usize);

/// See [`Landing`].
type LandingMut<'a, 'p, M> = (&'a mut Properties<M>, Segment<'p>, usize);

/// Whether the next `.name` piece can be applied to this value.
fn descend_check<M>(value: &PropertyValueEnum<M>) -> Result<(), ResolveErrorKind> {
    use PropertyValueEnum as V;
    match value {
        V::Struct(pointer) if *pointer.class_hash == 0 => Err(ResolveErrorKind::NullPointer),
        V::Struct(_) | V::Embedded(_) => Ok(()),
        other => Err(ResolveErrorKind::CannotDescend(other.kind())),
    }
}

/// The properties a `.name` piece looks a name up in. `None` exactly when [`descend_check`] fails.
fn properties_of<M>(value: &PropertyValueEnum<M>) -> Option<&Properties<M>> {
    use PropertyValueEnum as V;
    match value {
        V::Struct(pointer) if *pointer.class_hash == 0 => None,
        V::Struct(inner) | V::Embedded(values::Embedded(inner)) => Some(&inner.properties),
        _ => None,
    }
}

/// See [`properties_of`].
fn properties_of_mut<M>(value: &mut PropertyValueEnum<M>) -> Option<&mut Properties<M>> {
    use PropertyValueEnum as V;
    match value {
        V::Struct(pointer) if *pointer.class_hash == 0 => None,
        V::Struct(inner) | V::Embedded(values::Embedded(inner)) => Some(&mut inner.properties),
        _ => None,
    }
}

fn slot_for<M>(
    value: &PropertyValueEnum<M>,
    subscript: &Subscript<'_>,
) -> Result<Slot, ResolveErrorKind> {
    use PropertyValueEnum as V;

    match (value, subscript) {
        (V::Container(list), Subscript::Index(index))
        | (V::UnorderedContainer(values::UnorderedContainer(list)), Subscript::Index(index)) => {
            match (*index as usize) < list.len() {
                true => Ok(Slot::Item(*index as usize)),
                false => Err(ResolveErrorKind::IndexOutOfRange {
                    index: *index,
                    len: list.len(),
                }),
            }
        }
        (V::Optional(option), Subscript::Index(index)) => match (*index, option.is_some()) {
            (0, true) => Ok(Slot::OptionValue),
            _ => Err(ResolveErrorKind::IndexOutOfRange {
                index: *index,
                len: usize::from(option.is_some()),
            }),
        },
        (V::Map(map), Subscript::Key(literal)) => {
            let wanted = key_as(map.key_kind(), literal)
                .ok_or(ResolveErrorKind::InvalidKey(map.key_kind()))?;
            map.entries()
                .iter()
                .position(|(key, _)| key_eq(key, &wanted))
                .map(Slot::Entry)
                .ok_or(ResolveErrorKind::KeyNotFound)
        }
        (value, _) => Err(ResolveErrorKind::NotIndexable(value.kind())),
    }
}

fn take<M>(value: &PropertyValueEnum<M>, slot: Slot) -> &PropertyValueEnum<M> {
    use PropertyValueEnum as V;
    match (value, slot) {
        (V::Container(list), Slot::Item(index))
        | (V::UnorderedContainer(values::UnorderedContainer(list)), Slot::Item(index)) => {
            list.get(index).expect(MISSING)
        }
        (V::Optional(option), Slot::OptionValue) => option.value().expect(MISSING),
        (V::Map(map), Slot::Entry(index)) => &map.entries().get(index).expect(MISSING).1,
        _ => unreachable!("{MISSING}"),
    }
}

fn take_mut<M>(value: &mut PropertyValueEnum<M>, slot: Slot) -> ValueSlot<'_, M> {
    use PropertyValueEnum as V;
    match (value, slot) {
        (V::Container(list), Slot::Item(index))
        | (V::UnorderedContainer(values::UnorderedContainer(list)), Slot::Item(index)) => {
            list.slot(index).expect(MISSING)
        }
        (V::Optional(option), Slot::OptionValue) => option.slot().expect(MISSING),
        (V::Map(map), Slot::Entry(index)) => map.slot(index).expect(MISSING),
        _ => unreachable!("{MISSING}"),
    }
}

/// The key `literal` selects, as a value of `kind`.
///
/// The client parses the brace text as JSON and converts the result to the map's key type. A
/// number is written into an integer or float key, a string is taken as text for a string key and
/// hashed for a hash key. Anything else, including a number written with a fraction for an integer
/// key, has no conversion here. No shipped record uses a `{key}` subscript, so none of this is
/// attested in the wild.
fn key_as(kind: Kind, literal: &KeyLiteral<'_>) -> Option<PropertyValueEnum> {
    use KeyLiteral as L;

    Some(match (kind, literal) {
        (Kind::Bool, L::Bool(value)) => values::Bool::new(*value).into(),
        (Kind::BitBool, L::Bool(value)) => values::BitBool::new(*value).into(),
        (Kind::I8, L::Number(text)) => values::I8::new(text.parse().ok()?).into(),
        (Kind::U8, L::Number(text)) => values::U8::new(text.parse().ok()?).into(),
        (Kind::I16, L::Number(text)) => values::I16::new(text.parse().ok()?).into(),
        (Kind::U16, L::Number(text)) => values::U16::new(text.parse().ok()?).into(),
        (Kind::I32, L::Number(text)) => values::I32::new(text.parse().ok()?).into(),
        (Kind::U32, L::Number(text)) => values::U32::new(text.parse().ok()?).into(),
        (Kind::I64, L::Number(text)) => values::I64::new(text.parse().ok()?).into(),
        (Kind::U64, L::Number(text)) => values::U64::new(text.parse().ok()?).into(),
        (Kind::F32, L::Number(text)) => values::F32::new(text.parse().ok()?).into(),
        (Kind::String, L::String(text)) => values::String::new(text.to_string()).into(),
        (Kind::Hash, L::String(text)) => values::Hash::new(BinHash::hash_str(text)).into(),
        (Kind::Hash, L::Number(text)) => values::Hash::new(text.parse::<u32>().ok()?).into(),
        (Kind::WadChunkLink, L::String(text)) => {
            values::WadChunkLink::new(WadHash::hash_str(text)).into()
        }
        (Kind::WadChunkLink, L::Number(text)) => {
            values::WadChunkLink::new(text.parse::<u64>().ok()?).into()
        }
        _ => return None,
    })
}

macro_rules! match_key {
    ($key:expr, $wanted:expr, [$($variant:ident,)*]) => {
        match ($key, $wanted) {
            $((PropertyValueEnum::$variant(key), PropertyValueEnum::$variant(wanted)) => {
                key.value == wanted.value
            })*
            _ => false,
        }
    };
}

/// Whether a map key equals the value [`key_as`] produced, ignoring metadata.
fn key_eq<M>(key: &PropertyValueEnum<M>, wanted: &PropertyValueEnum) -> bool {
    match_key!(
        key,
        wanted,
        [
            Bool,
            BitBool,
            I8,
            U8,
            I16,
            U16,
            I32,
            U32,
            I64,
            U64,
            F32,
            String,
            Hash,
            WadChunkLink,
        ]
    )
}

/// Looks `segment` up and applies its subscript.
fn step<'a, M>(
    properties: &'a Properties<M>,
    segment: &Segment<'_>,
    index: usize,
) -> Result<&'a PropertyValueEnum<M>, ResolveError> {
    let name_hash = segment.name_hash();
    let value = properties
        .get(&name_hash)
        .ok_or_else(|| ResolveError::new(index, ResolveErrorKind::MissingProperty(name_hash)))?;

    match &segment.subscript {
        None => Ok(value),
        Some(subscript) => {
            let slot = slot_for(value, subscript).map_err(|kind| ResolveError::new(index, kind))?;
            Ok(take(value, slot))
        }
    }
}

/// See [`step`].
fn step_mut<'a, M>(
    properties: &'a mut Properties<M>,
    segment: &Segment<'_>,
    index: usize,
) -> Result<ValueSlot<'a, M>, ResolveError> {
    let name_hash = segment.name_hash();
    let value = properties
        .get_mut(&name_hash)
        .ok_or_else(|| ResolveError::new(index, ResolveErrorKind::MissingProperty(name_hash)))?;

    match &segment.subscript {
        None => Ok(ValueSlot::free(value)),
        Some(subscript) => {
            let slot = slot_for(value, subscript).map_err(|kind| ResolveError::new(index, kind))?;
            Ok(take_mut(value, slot))
        }
    }
}

/// Walks every segment but the last, and returns the properties the last one is looked up in.
///
/// Splitting the walk here is what lets `patch` treat an absent leaf differently from an absent
/// step on the way down.
fn locate<'a, 'p, M>(
    properties: &'a Properties<M>,
    path: &'p PropertyPath,
) -> Result<Landing<'a, 'p, M>, ResolveError> {
    let mut properties = properties;
    let mut segments = path.segments().enumerate().peekable();

    loop {
        let (index, segment) = segments
            .next()
            .expect("a PropertyPath has at least one segment");
        if segments.peek().is_none() {
            return Ok((properties, segment, index));
        }

        let value = step(properties, &segment, index)?;
        descend_check(value).map_err(|kind| ResolveError::new(index + 1, kind))?;
        properties = properties_of(value).expect("descend_check accepted this value");
    }
}

/// See [`locate`].
fn locate_mut<'a, 'p, M>(
    properties: &'a mut Properties<M>,
    path: &'p PropertyPath,
) -> Result<LandingMut<'a, 'p, M>, ResolveError> {
    let mut properties = properties;
    let mut segments = path.segments().enumerate().peekable();

    loop {
        let (index, segment) = segments
            .next()
            .expect("a PropertyPath has at least one segment");
        if segments.peek().is_none() {
            return Ok((properties, segment, index));
        }

        let value = step_mut(properties, &segment, index)?.into_inner();
        descend_check(value).map_err(|kind| ResolveError::new(index + 1, kind))?;
        properties = properties_of_mut(value).expect("descend_check accepted this value");
    }
}

pub(crate) fn walk<'a, M>(
    properties: &'a Properties<M>,
    path: &PropertyPath,
) -> Result<&'a PropertyValueEnum<M>, ResolveError> {
    let (properties, segment, index) = locate(properties, path)?;
    step(properties, &segment, index)
}

pub(crate) fn walk_mut<'a, M>(
    properties: &'a mut Properties<M>,
    path: &PropertyPath,
) -> Result<ValueSlot<'a, M>, ResolveError> {
    let (properties, segment, index) = locate_mut(properties, path)?;
    step_mut(properties, &segment, index)
}

pub(crate) fn patch_in<M>(
    properties: &mut Properties<M>,
    path: &PropertyPath,
    value: PropertyValueEnum<M>,
) -> Result<Option<PropertyValueEnum<M>>, PatchError> {
    let (properties, segment, index) = locate_mut(properties, path)?;
    let name_hash = segment.name_hash();

    if !properties.contains_key(&name_hash) {
        // The client has no insert: it patches a live object on which every property the class
        // declares already exists, holding whatever its constructor left there. Creating the leaf
        // is the closest a serialized tree gets, and only when the segment names it outright - a
        // subscript needs something to subscript.
        return match segment.subscript {
            None => {
                properties.insert(name_hash, value);
                Ok(None)
            }
            Some(_) => {
                Err(ResolveError::new(index, ResolveErrorKind::MissingProperty(name_hash)).into())
            }
        };
    }

    let mut slot = step_mut(properties, &segment, index)?;
    let expected = ValueShape::of(slot.get());
    let found = ValueShape::of(&value);
    if !expected.matches(&found) {
        return Err(PatchError::TypeMismatch { expected, found });
    }

    Ok(Some(slot.set(value).expect(
        "the shapes match, so the kinds do and no slot can reject the value",
    )))
}

/// Whether [`patch_in`] would apply, and whether it would insert, without touching anything.
pub(crate) fn probe<M>(
    properties: &Properties<M>,
    path: &PropertyPath,
    found: ValueShape,
) -> Result<bool, PatchError> {
    let (properties, segment, index) = locate(properties, path)?;
    let name_hash = segment.name_hash();

    let Some(value) = properties.get(&name_hash) else {
        return match segment.subscript {
            None => Ok(true),
            Some(_) => {
                Err(ResolveError::new(index, ResolveErrorKind::MissingProperty(name_hash)).into())
            }
        };
    };

    let leaf = match &segment.subscript {
        None => value,
        Some(subscript) => {
            let slot = slot_for(value, subscript).map_err(|kind| ResolveError::new(index, kind))?;
            take(value, slot)
        }
    };

    let expected = ValueShape::of(leaf);
    match expected.matches(&found) {
        true => Ok(false),
        false => Err(PatchError::TypeMismatch { expected, found }),
    }
}

impl<M> BinObject<M> {
    /// The value at `path` inside this object.
    ///
    /// # Errors
    ///
    /// [`ResolveError`] naming the segment that could not be applied.
    ///
    /// # Examples
    ///
    /// ```
    /// use ltk_meta::{
    ///     path::PropertyPath,
    ///     property::{values, NoMeta},
    ///     BinObject,
    /// };
    ///
    /// let path = PropertyPath::new("Elements[1]")?;
    /// // A name selects a property by the hash of its lowercased text.
    /// let elements = path.segments().next().unwrap().name_hash();
    ///
    /// let object = BinObject::<NoMeta>::builder(0x1234, 0x5678)
    ///     .property(
    ///         elements,
    ///         values::Container::from(vec![values::I32::new(10), values::I32::new(20)]),
    ///     )
    ///     .build();
    ///
    /// assert_eq!(object.resolve(&path)?, &values::I32::new(20).into());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError> {
        walk(&self.properties, path)
    }

    /// A mutable handle on the value at `path` inside this object.
    ///
    /// This is the unchecked way in: it hands back whatever is there and applies no type rule.
    /// [`BinObject::patch`] is the one that reproduces the client.
    ///
    /// # Errors
    ///
    /// The same as [`BinObject::resolve`].
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError> {
        walk_mut(&mut self.properties, path)
    }

    /// Sets the property at `path` the way a `PTCH` record does.
    ///
    /// Returns the value that was replaced, or `None` when the leaf did not exist and was
    /// created. Creating the leaf is what makes 2,455 of Riot's shipped records apply: the client
    /// patches a live object on which every property its class declares already exists, so there
    /// the property is never absent. A leaf named by a subscript is never created, and neither is
    /// a step on the way down: an absent intermediate is a [`ResolveErrorKind::MissingProperty`],
    /// where the client would patch into the default the constructor left behind.
    ///
    /// # Errors
    ///
    /// [`PatchError::Resolve`] if `path` does not name a property, or
    /// [`PatchError::TypeMismatch`] if it names one of a different shape, in which case nothing
    /// is changed.
    pub fn patch(
        &mut self,
        path: &PropertyPath,
        value: PropertyValueEnum<M>,
    ) -> Result<Option<PropertyValueEnum<M>>, PatchError> {
        patch_in(&mut self.properties, path, value)
    }

    /// Whether [`BinObject::patch`] would apply, and whether it would create the leaf.
    pub(crate) fn probe(
        &self,
        path: &PropertyPath,
        value: &PropertyValueEnum<M>,
    ) -> Result<bool, PatchError> {
        probe(&self.properties, path, ValueShape::of(value))
    }
}

impl<M> values::Struct<M> {
    /// The value at `path` inside this pointer's target.
    ///
    /// # Errors
    ///
    /// [`ResolveErrorKind::NullPointer`] at segment 0 if the class hash is 0, otherwise the same
    /// as [`BinObject::resolve`].
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError> {
        self.null_check()?;
        walk(&self.properties, path)
    }

    /// See [`values::Struct::resolve`] and [`BinObject::resolve_mut`].
    ///
    /// # Errors
    ///
    /// The same as [`values::Struct::resolve`].
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError> {
        self.null_check()?;
        walk_mut(&mut self.properties, path)
    }

    /// See [`BinObject::patch`].
    ///
    /// # Errors
    ///
    /// The same as [`BinObject::patch`], plus [`ResolveErrorKind::NullPointer`].
    pub fn patch(
        &mut self,
        path: &PropertyPath,
        value: PropertyValueEnum<M>,
    ) -> Result<Option<PropertyValueEnum<M>>, PatchError> {
        self.null_check()?;
        patch_in(&mut self.properties, path, value)
    }

    fn null_check(&self) -> Result<(), ResolveError> {
        match *self.class_hash {
            0 => Err(ResolveError::new(0, ResolveErrorKind::NullPointer)),
            _ => Ok(()),
        }
    }
}

impl<M> values::Embedded<M> {
    /// The value at `path` inside this embed. See [`BinObject::resolve`].
    ///
    /// # Errors
    ///
    /// The same as [`BinObject::resolve`]. An embed is inline, so unlike a pointer it is never
    /// null and a class hash of 0 is not special here.
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError> {
        walk(&self.0.properties, path)
    }

    /// See [`BinObject::resolve_mut`].
    ///
    /// # Errors
    ///
    /// The same as [`values::Embedded::resolve`].
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError> {
        walk_mut(&mut self.0.properties, path)
    }

    /// See [`BinObject::patch`].
    ///
    /// # Errors
    ///
    /// The same as [`BinObject::patch`].
    pub fn patch(
        &mut self,
        path: &PropertyPath,
        value: PropertyValueEnum<M>,
    ) -> Result<Option<PropertyValueEnum<M>>, PatchError> {
        patch_in(&mut self.0.properties, path, value)
    }
}

impl<M> PropertyValueEnum<M> {
    /// The value at `path` relative to this one.
    ///
    /// The first segment applies to this value, so it has to be a pointer or an embed for a path
    /// to go anywhere at all.
    ///
    /// # Errors
    ///
    /// [`ResolveErrorKind::CannotDescend`] or [`ResolveErrorKind::NullPointer`] at segment 0 if
    /// this value cannot be descended into, otherwise the same as [`BinObject::resolve`].
    pub fn resolve(&self, path: &PropertyPath) -> Result<&PropertyValueEnum<M>, ResolveError> {
        descend_check(self).map_err(|kind| ResolveError::new(0, kind))?;
        walk(
            properties_of(self).expect("descend_check accepted this value"),
            path,
        )
    }

    /// See [`PropertyValueEnum::resolve`] and [`BinObject::resolve_mut`].
    ///
    /// # Errors
    ///
    /// The same as [`PropertyValueEnum::resolve`].
    pub fn resolve_mut(&mut self, path: &PropertyPath) -> Result<ValueSlot<'_, M>, ResolveError> {
        descend_check(self).map_err(|kind| ResolveError::new(0, kind))?;
        walk_mut(
            properties_of_mut(self).expect("descend_check accepted this value"),
            path,
        )
    }
}

impl<M> Bin<M> {
    /// The value at `path` inside object `object_hash`.
    ///
    /// # Errors
    ///
    /// [`ResolveErrorKind::MissingObject`] if the bin has no such object, otherwise the same as
    /// [`BinObject::resolve`].
    pub fn resolve(
        &self,
        object_hash: impl Into<BinHash>,
        path: &PropertyPath,
    ) -> Result<&PropertyValueEnum<M>, ResolveError> {
        self.object(object_hash)?.resolve(path)
    }

    /// See [`Bin::resolve`] and [`BinObject::resolve_mut`].
    ///
    /// # Errors
    ///
    /// The same as [`Bin::resolve`].
    pub fn resolve_mut(
        &mut self,
        object_hash: impl Into<BinHash>,
        path: &PropertyPath,
    ) -> Result<ValueSlot<'_, M>, ResolveError> {
        let object_hash = object_hash.into();
        self.objects
            .get_mut(&object_hash)
            .ok_or_else(|| ResolveError::new(0, ResolveErrorKind::MissingObject(object_hash)))?
            .resolve_mut(path)
    }

    /// Sets a property inside object `object_hash` the way a `PTCH` record does.
    ///
    /// See [`BinObject::patch`].
    ///
    /// # Errors
    ///
    /// The same as [`BinObject::patch`], plus [`ResolveErrorKind::MissingObject`].
    pub fn patch(
        &mut self,
        object_hash: impl Into<BinHash>,
        path: &PropertyPath,
        value: PropertyValueEnum<M>,
    ) -> Result<Option<PropertyValueEnum<M>>, PatchError> {
        let object_hash = object_hash.into();
        let object = self
            .objects
            .get_mut(&object_hash)
            .ok_or_else(|| ResolveError::new(0, ResolveErrorKind::MissingObject(object_hash)))?;

        object.patch(path, value)
    }

    fn object(&self, object_hash: impl Into<BinHash>) -> Result<&BinObject<M>, ResolveError> {
        let object_hash = object_hash.into();
        self.objects
            .get(&object_hash)
            .ok_or_else(|| ResolveError::new(0, ResolveErrorKind::MissingObject(object_hash)))
    }
}
