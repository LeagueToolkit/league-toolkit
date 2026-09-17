//! The difference between two bins as a patch: [`Bin::diff`].

use std::{
    collections::{HashMap, HashSet},
    fmt,
};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{
    merge::{combines, key_index, map_keys},
    path::{FieldNames, MapKey, Nameless, NamelessKind, PropertyPath, ValuePath, ValueShape},
    property::values,
    walk::{Trail, TrailSegment},
    Bin, BinObject, BinOverride, PropertyPatch, PropertyValueEnum,
};

/// How a diff treats what the edit leaves out.
///
/// # Examples
///
/// ```
/// use ltk_meta::DiffOptions;
///
/// let mut options = DiffOptions::default();
/// options.deletions = true;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub struct DiffOptions {
    /// An object in the base and not in the edit goes on [`BinOverride::deleted`].
    ///
    /// Off by default. A mod that omits an object is not asking for it to be deleted. A tool
    /// authoring a deliberate patch turns it on.
    pub deletions: bool,
}

/// A place the record language could not carry what the diff found there.
///
/// The record that covers the position is at an ancestor of it, or the whole object went into
/// [`BinOverride::objects`]. That record carries the base merged with the edit. The patch
/// applied to the base it was made from equals [`Bin::merge`]. Applied to another base, it
/// overwrites whatever that base holds anywhere inside the record's value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Lift {
    /// The edit adds `keys` entries to the map at `at`. No record inserts a map entry.
    MapInsert {
        /// The object the map is in.
        object_hash: BinHash,
        /// The map.
        at: ValuePath,
        /// How many entries the edit adds.
        keys: usize,
    },
    /// No client path spells `at`: a field on it has no name in the table, or a key has no
    /// literal.
    Nameless {
        /// The object the position is in.
        object_hash: BinHash,
        /// The position a record could not be written at.
        at: ValuePath,
        /// The first segment that cannot be spelled, and why.
        cause: Nameless,
    },
    /// The two sides hold different shapes at `at`. No record changes the shape of a value: the
    /// patch type rule skips it.
    Mismatch {
        /// The object the position is in.
        object_hash: BinHash,
        /// The position whose shapes differ.
        at: ValuePath,
    },
}

impl Lift {
    /// The object the lifted position is in.
    #[must_use]
    pub fn object_hash(&self) -> BinHash {
        match self {
            Self::MapInsert { object_hash, .. }
            | Self::Nameless { object_hash, .. }
            | Self::Mismatch { object_hash, .. } => *object_hash,
        }
    }

    /// The lifted position, inside [`Lift::object_hash`].
    #[must_use]
    pub fn at(&self) -> &ValuePath {
        match self {
            Self::MapInsert { at, .. } | Self::Nameless { at, .. } | Self::Mismatch { at, .. } => {
                at
            }
        }
    }
}

/// `"01000001 1e6ba0c4: 2 map entries inserted"`.
impl fmt::Display for Lift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x} {}: ", self.object_hash(), self.at())?;
        match self {
            Self::MapInsert { keys, .. } => write!(f, "{keys} map entries inserted"),
            Self::Nameless { cause, .. } => write!(f, "nameless, {cause}"),
            Self::Mismatch { .. } => f.write_str("shapes differ"),
        }
    }
}

/// What a diff produced.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiffReport {
    /// Records emitted into [`BinOverride::patches`].
    pub records: usize,
    /// Objects taken whole into [`BinOverride::objects`], in the edit's order.
    pub objects: Vec<BinHash>,
    /// Objects put on [`BinOverride::deleted`]. Empty unless [`DiffOptions::deletions`].
    pub deleted: Vec<BinHash>,
    /// Dependencies the edit declares and the base does not. A patch declares no dependency.
    pub dependencies: Vec<String>,
    /// Every place the record language could not carry the difference, in walk order.
    pub lifted: Vec<Lift>,
}

/// `"3 records, 1 object, 0 deleted, 0 dependencies, 2 lifted"`.
impl fmt::Display for DiffReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} records, {} objects, {} deleted, {} dependencies, {} lifted",
            self.records,
            self.objects.len(),
            self.deleted.len(),
            self.dependencies.len(),
            self.lifted.len(),
        )
    }
}

impl<M: Clone + PartialEq> Bin<M> {
    /// The patch that turns this bin into `edited`, as far as records can say it.
    ///
    /// The patch applied to this bin equals `self.merge(edited)`. A difference goes into a
    /// record at the position it is found at where the record language carries it there, and
    /// otherwise into a record at the nearest ancestor that carries it, holding this bin's value
    /// merged with the edit's. At the root that ancestor is the whole object, which goes into
    /// [`BinOverride::objects`]. Every such place is a [`Lift`] in the report.
    ///
    /// Record paths are spelled with `names`. An object only the edit has goes into
    /// [`BinOverride::objects`] whole. An object on both sides with different classes does too,
    /// with a [`Lift::Mismatch`] at its root.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use ltk_hash::{BinHash, Hash as _};
    /// use ltk_meta::concrete::{values, Bin, BinObject};
    ///
    /// let size = BinHash::hash_str("Size");
    /// let names = HashMap::from([(size, "Size".to_owned())]);
    /// let object = |value| {
    ///     Bin::builder()
    ///         .object(BinObject::builder(0x1u32, 0xc1u32).property(size, values::I32::new(value)).build())
    ///         .build()
    /// };
    ///
    /// let (patch, report) = object(1).diff(&object(2), &names);
    /// assert_eq!(patch.patches[0].path.as_str(), "Size");
    /// assert!(report.lifted.is_empty());
    /// ```
    pub fn diff(&self, edited: &Self, names: &dyn FieldNames) -> (BinOverride<M>, DiffReport) {
        self.diff_with(edited, names, &DiffOptions::default())
    }

    /// The patch that turns this bin into `edited`, under `options`.
    ///
    /// See [`Bin::diff`] and [`DiffOptions`].
    pub fn diff_with(
        &self,
        edited: &Self,
        names: &dyn FieldNames,
        options: &DiffOptions,
    ) -> (BinOverride<M>, DiffReport) {
        let mut patch = BinOverride::new();
        let mut differ = Differ {
            names,
            object_hash: BinHash(0),
            trail: Trail::new(),
            report: DiffReport::default(),
        };

        for (object_hash, object) in &edited.objects {
            let Some(base) = self.objects.get(object_hash) else {
                differ.take_object(&mut patch, object.clone());
                continue;
            };
            differ.object_hash = *object_hash;
            differ.trail.clear();
            if base.class_hash != object.class_hash {
                differ.lift(|object_hash, at| Lift::Mismatch { object_hash, at });
                differ.take_object(&mut patch, object.clone());
                continue;
            }

            let mut records = Vec::new();
            match differ.properties(
                &base.properties,
                &object.properties,
                object.class_hash,
                &mut records,
            ) {
                Ok(()) => {
                    differ.report.records += records.len();
                    patch
                        .patches
                        .extend(records.into_iter().map(|(path, value)| PropertyPatch {
                            object_hash: *object_hash,
                            path,
                            value,
                        }));
                }
                Err(_) => {
                    let mut merged = base.clone();
                    merged.merge(object);
                    differ.take_object(&mut patch, merged);
                }
            }
        }

        if options.deletions {
            for object_hash in self.objects.keys() {
                if !edited.objects.contains_key(object_hash) {
                    patch.deleted.push(*object_hash);
                    differ.report.deleted.push(*object_hash);
                }
            }
        }

        differ.report.dependencies = edited
            .dependencies
            .iter()
            .filter(|dependency| !self.dependencies.contains(dependency))
            .cloned()
            .collect();

        (patch, differ.report)
    }
}

/// A record not yet attached to its object: a path and the value it writes.
type Pending<M> = (PropertyPath, PropertyValueEnum<M>);

/// The depth a difference has to be recorded at or above: a record at a position whose trail is
/// longer than this cannot carry it. 0 is the whole object.
type Escalate = usize;

/// One diff: the name table, the report it builds and the trail it reports positions with.
struct Differ<'n, 'e, M> {
    names: &'n dyn FieldNames,
    object_hash: BinHash,
    trail: Trail<&'e PropertyValueEnum<M>>,
    report: DiffReport,
}

impl<'e, M: Clone + PartialEq> Differ<'_, 'e, M> {
    fn take_object(&mut self, patch: &mut BinOverride<M>, object: BinObject<M>) {
        self.report.objects.push(object.path_hash);
        patch.objects.insert(object.path_hash, object);
    }

    /// The trail's position, as the owned address a report holds.
    fn at(&self) -> ValuePath {
        self.trail
            .to_value_path()
            .expect("every key on the trail converted when its map was indexed")
    }

    fn lift(&mut self, lift: impl FnOnce(BinHash, ValuePath) -> Lift) {
        let lift = lift(self.object_hash, self.at());
        self.report.lifted.push(lift);
    }

    fn properties(
        &mut self,
        base: &IndexMap<BinHash, PropertyValueEnum<M>>,
        edited: &'e IndexMap<BinHash, PropertyValueEnum<M>>,
        class: BinHash,
        records: &mut Vec<Pending<M>>,
    ) -> Result<(), Escalate> {
        let mut escalate = None;
        for (field, value) in edited {
            self.trail.push_field(*field, class);
            let found = match base.get(field) {
                Some(existing) => self.value(existing, value, records),
                None => self.record(records, || value.clone()),
            };
            self.trail.pop();
            escalate = shallowest(escalate, found.err());
        }
        escalate.map_or(Ok(()), Err)
    }

    /// Diffs the value at the trail's position.
    fn value(
        &mut self,
        base: &PropertyValueEnum<M>,
        edited: &'e PropertyValueEnum<M>,
        records: &mut Vec<Pending<M>>,
    ) -> Result<(), Escalate> {
        use PropertyValueEnum as V;

        if base == edited {
            return Ok(());
        }
        let mark = records.len();
        let escalate = match (base, edited) {
            (V::Struct(b), V::Struct(e))
            | (V::Embedded(values::Embedded(b)), V::Embedded(values::Embedded(e)))
                if combines(b, e) =>
            {
                self.properties(&b.properties, &e.properties, e.class_hash, records)
                    .err()
            }
            (V::Map(b), V::Map(e))
                if b.key_kind() == e.key_kind() && b.value_kind() == e.value_kind() =>
            {
                match self.map(b, e, records) {
                    Some(found) => found.err(),
                    None => return self.replace(base, edited, records),
                }
            }
            (V::Optional(b), V::Optional(e)) if b.item_kind() == e.item_kind() => {
                match (b.value(), e.value()) {
                    (Some(b), Some(e)) => {
                        self.trail.push(TrailSegment::Index(0));
                        let found = self.value(b, e, records);
                        self.trail.pop();
                        found.err()
                    }
                    _ => return self.replace(base, edited, records),
                }
            }
            _ => return self.replace(base, edited, records),
        };

        let Some(to) = escalate else {
            return Ok(());
        };
        records.truncate(mark);
        if self.trail.len() > to {
            return Err(to);
        }
        self.record(records, || merged(base, edited))
    }

    /// Diffs two maps of the same kinds entry by entry. `None` when an entry breaks its map's
    /// kinds or has a key that does not convert to a [`MapKey`].
    ///
    /// A key the edit repeats is diffed against the base's entry with its earlier occurrences
    /// merged over it, as [`Bin::merge`] applies them in order.
    fn map(
        &mut self,
        base: &values::Map<M>,
        edited: &'e values::Map<M>,
        records: &mut Vec<Pending<M>>,
    ) -> Option<Result<(), Escalate>> {
        let index = key_index(base)?;
        let keys = map_keys(edited)?;
        let mut occurrences: HashMap<&MapKey, usize> = HashMap::new();
        for key in &keys {
            *occurrences.entry(key).or_default() += 1;
        }

        let mut escalate = None;
        let mut inserted = HashSet::new();
        // The base's entries a repeated key has written, as the merge leaves them.
        let mut written: HashMap<usize, PropertyValueEnum<M>> = HashMap::new();
        for ((key, value), map_key) in edited.entries().iter().zip(&keys) {
            let Some(at) = index.get(map_key).copied() else {
                inserted.insert(map_key);
                continue;
            };
            let existing = written.get(&at).unwrap_or(&base.entries()[at].1);
            self.trail.push(TrailSegment::Key(key));
            let found = if existing == value {
                Ok(())
            } else if shadowed(base, at) {
                self.shadowed_key(map_key)
            } else {
                self.value(existing, value, records)
            };
            self.trail.pop();
            escalate = shallowest(escalate, found.err());
            if occurrences[map_key] > 1 {
                let next = merged(existing, value);
                written.insert(at, next);
            }
        }
        if !inserted.is_empty() {
            self.lift(|object_hash, at| Lift::MapInsert {
                object_hash,
                at,
                keys: inserted.len(),
            });
            escalate = shallowest(escalate, Some(self.trail.len()));
        }
        Some(escalate.map_or(Ok(()), Err))
    }

    /// A [`Lift::Nameless`] at an entry whose key literal resolves to an earlier entry,
    /// escalating to the map.
    fn shadowed_key(&mut self, key: &MapKey) -> Result<(), Escalate> {
        let segment = self.trail.len() - 1;
        self.lift(|object_hash, at| Lift::Nameless {
            object_hash,
            at,
            cause: Nameless {
                segment,
                kind: NamelessKind::Key(key.kind()),
            },
        });
        Err(segment)
    }

    /// A value that differs and does not combine: a record of the edit's value where the type rule
    /// accepts one, else a [`Lift::Mismatch`] escalating to the parent.
    fn replace(
        &mut self,
        base: &PropertyValueEnum<M>,
        edited: &PropertyValueEnum<M>,
        records: &mut Vec<Pending<M>>,
    ) -> Result<(), Escalate> {
        if ValueShape::of(base).matches(&ValueShape::of(edited)) {
            return self.record(records, || edited.clone());
        }
        self.lift(|object_hash, at| Lift::Mismatch { object_hash, at });
        Err(self.trail.len() - 1)
    }

    /// A record of `value` at the trail's position, or a [`Lift::Nameless`] escalating to the
    /// longest prefix every segment of which is spelled.
    fn record(
        &mut self,
        records: &mut Vec<Pending<M>>,
        value: impl FnOnce() -> PropertyValueEnum<M>,
    ) -> Result<(), Escalate> {
        let at = self.at();
        match at.to_property_path(self.names) {
            Ok(path) => {
                records.push((path, value()));
                Ok(())
            }
            Err(cause) => {
                let to = cause.segment;
                self.report.lifted.push(Lift::Nameless {
                    object_hash: self.object_hash,
                    at,
                    cause,
                });
                Err(to)
            }
        }
    }
}

/// Whether the `{key}` literal of entry `at` resolves to an earlier entry of `map`.
///
/// The resolver matches a float key with `==`, and a key matches by its bits here. `0.0` and
/// `-0.0` are the two keys that are equal under `==` with different bits: the later of the two
/// is shadowed. A `NaN` key has no literal at all.
fn shadowed<M>(map: &values::Map<M>, at: usize) -> bool {
    let PropertyValueEnum::F32(key) = &map.entries()[at].0 else {
        return false;
    };
    key.value == 0.0
        && map.entries()[..at].iter().any(|(earlier, _)| {
            matches!(earlier, PropertyValueEnum::F32(e) if e.value == 0.0 && e.value.to_bits() != key.value.to_bits())
        })
}

/// `base` with `edited` merged over it.
fn merged<M: Clone + PartialEq>(
    base: &PropertyValueEnum<M>,
    edited: &PropertyValueEnum<M>,
) -> PropertyValueEnum<M> {
    let mut merged = base.clone();
    merged.merge(edited);
    merged
}

/// The shallower of two escalations.
fn shallowest(a: Option<Escalate>, b: Option<Escalate>) -> Option<Escalate> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
