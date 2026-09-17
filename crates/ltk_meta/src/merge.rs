//! Layering one bin over another: [`Bin::merge`].

use std::{collections::HashMap, fmt, mem};

use indexmap::IndexMap;
use ltk_hash::BinHash;

use crate::{
    path::{MapKey, ValuePath, ValueShape},
    property::{values, NoMeta, ValueSlot},
    walk::{Trail, TrailSegment},
    Bin, BinObject, PropertyValueEnum,
};

/// What a merge did: what it overwrote, and what it added.
///
/// # Examples
///
/// ```
/// use ltk_meta::concrete::{values, Bin, BinObject};
///
/// let mut base = Bin::builder()
///     .object(BinObject::builder(0x1u32, 0xc1u32).property(0xau32, values::I32::new(1)).build())
///     .build();
/// let edited = Bin::builder()
///     .object(BinObject::builder(0x1u32, 0xc1u32).property(0xbu32, values::I32::new(2)).build())
///     .build();
///
/// let report = base.merge(&edited);
/// assert_eq!(report.inserted, 1);
/// assert_eq!(base.objects[&ltk_hash::BinHash(0x1)].properties.len(), 2);
/// ```
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MergeReport<M = NoMeta> {
    /// Objects taken whole from the edit: the base had no object with the hash.
    pub objects_added: Vec<BinHash>,
    /// Objects on both sides with the same class, combined property by property.
    pub objects_merged: Vec<BinHash>,
    /// Objects on both sides with different classes. The edit's object replaced each, and each
    /// is held here as the base held it.
    pub objects_replaced: Vec<BinObject<M>>,
    /// Every value the edit overwrote inside a combined object, with the value the base held.
    pub replaced: Vec<Replaced<M>>,
    /// Properties the base did not have, inserted from the edit.
    pub inserted: usize,
    /// Map entries the base did not have, appended from the edit.
    pub keys_inserted: usize,
}

impl<M> Default for MergeReport<M> {
    fn default() -> Self {
        Self {
            objects_added: Vec::new(),
            objects_merged: Vec::new(),
            objects_replaced: Vec::new(),
            replaced: Vec::new(),
            inserted: 0,
            keys_inserted: 0,
        }
    }
}

impl<M> MergeReport<M> {
    /// Whether the merge left the base as it was: nothing added, replaced or inserted.
    #[must_use]
    pub fn is_unchanged(&self) -> bool {
        self.objects_added.is_empty()
            && self.objects_replaced.is_empty()
            && self.replaced.is_empty()
            && self.inserted == 0
            && self.keys_inserted == 0
    }
}

/// `"1 added, 2 merged, 0 replaced; 3 values replaced (1 mismatched), 4 inserted, 5 keys
/// inserted"`.
impl<M> fmt::Display for MergeReport<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} added, {} merged, {} replaced; {} values replaced ({} mismatched), {} inserted, \
             {} keys inserted",
            self.objects_added.len(),
            self.objects_merged.len(),
            self.objects_replaced.len(),
            self.replaced.len(),
            self.replaced.iter().filter(|r| r.mismatched).count(),
            self.inserted,
            self.keys_inserted,
        )
    }
}

/// One value the edit overwrote.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Replaced<M = NoMeta> {
    /// The object the value is in. 0 for a merge of a `Struct` or a value, which is in no object.
    pub object_hash: BinHash,
    /// Where the value is, inside that object.
    pub at: ValuePath,
    /// What the base held. Moved out of the base, never cloned.
    pub was: PropertyValueEnum<M>,
    /// Whether the two sides held different shapes: a different kind, a container, optional or
    /// map declaring different kinds, or a `Struct` or `Embedded` of a different class.
    ///
    /// The client compares a value's tag with the property's registered tag by exact equality,
    /// and discards a value whose tag differs with no error. Riot changes a property's type in
    /// place, and a mod value that predates the change mismatches the game's value here.
    pub mismatched: bool,
}

impl<M> fmt::Display for Replaced<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:08x} {}", self.object_hash, self.at)?;
        if self.mismatched {
            f.write_str(" (mismatched)")?;
        }
        Ok(())
    }
}

impl<M: Clone + PartialEq> Bin<M> {
    /// Layers `edited` over this bin, in place.
    ///
    /// The edit wins at every value it reaches, and whatever only this bin holds survives. An
    /// object only the edit has is added. An object on both sides is combined property by
    /// property when the classes agree, and replaced whole when they do not. Dependencies merge
    /// as a union: this bin's in order, then the edit's new ones.
    ///
    /// A merge never refuses.
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M> {
        let mut merger = Merger::new();
        for (object_hash, object) in &edited.objects {
            match self.objects.get_mut(object_hash) {
                Some(base) => merger.object(base, object),
                None => {
                    self.objects.insert(*object_hash, object.clone());
                    merger.report.objects_added.push(*object_hash);
                }
            }
        }
        for dependency in &edited.dependencies {
            if !self.dependencies.contains(dependency) {
                self.dependencies.push(dependency.clone());
            }
        }
        merger.report
    }
}

impl<M: Clone + PartialEq> BinObject<M> {
    /// Layers `edited` over this object, in place. See [`Bin::merge`].
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M> {
        let mut merger = Merger::new();
        merger.object(self, edited);
        merger.report
    }
}

impl<M: Clone + PartialEq> values::Struct<M> {
    /// Layers `edited` over this `Struct`, in place. See [`Bin::merge`].
    ///
    /// Two structs of the same class that is not 0 combine property by property. Anything else
    /// that differs is replaced whole.
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M> {
        let mut merger = Merger::new();
        if combines(self, edited) {
            merger.properties(&mut self.properties, &edited.properties, edited.class_hash);
        } else if self != edited {
            let was = mem::replace(self, edited.clone());
            let mismatched = was.class_hash != edited.class_hash;
            merger.record(PropertyValueEnum::Struct(was), mismatched);
        }
        merger.report
    }
}

impl<M: Clone + PartialEq> PropertyValueEnum<M> {
    /// Layers `edited` over this value, in place. See [`Bin::merge`].
    pub fn merge(&mut self, edited: &Self) -> MergeReport<M> {
        let mut merger = Merger::new();
        merger.value(self, edited);
        merger.report
    }
}

/// Whether two nodes combine property by property: the same class, and not the null pointer.
fn combines<M>(base: &values::Struct<M>, edited: &values::Struct<M>) -> bool {
    base.class_hash == edited.class_hash && *base.class_hash != 0
}

/// Whether two values differ in shape: [`ValueShape`], or the class of a `Struct`.
pub(crate) fn mismatched<M>(base: &PropertyValueEnum<M>, edited: &PropertyValueEnum<M>) -> bool {
    let classes_differ = match (base, edited) {
        (PropertyValueEnum::Struct(b), PropertyValueEnum::Struct(e)) => {
            b.class_hash != e.class_hash
        }
        _ => false,
    };
    classes_differ || !ValueShape::of(base).matches(&ValueShape::of(edited))
}

/// The key of every entry of `map`, by position of its first occurrence. `None` when a key does
/// not convert, which only a map built around its own constructor holds.
pub(crate) fn key_index<M>(map: &values::Map<M>) -> Option<HashMap<MapKey, usize>> {
    let mut index = HashMap::with_capacity(map.entries().len());
    for (at, (key, _)) in map.entries().iter().enumerate() {
        index.entry(MapKey::try_from(key).ok()?).or_insert(at);
    }
    Some(index)
}

/// One merge: the report it builds and the trail it reports positions with.
struct Merger<'e, M> {
    object_hash: BinHash,
    trail: Trail<&'e PropertyValueEnum<M>>,
    report: MergeReport<M>,
}

impl<'e, M: Clone + PartialEq> Merger<'e, M> {
    fn new() -> Self {
        Self {
            object_hash: BinHash(0),
            trail: Trail::new(),
            report: MergeReport::default(),
        }
    }

    fn object(&mut self, base: &mut BinObject<M>, edited: &'e BinObject<M>) {
        if base.class_hash != edited.class_hash {
            let was = mem::replace(base, edited.clone());
            self.report.objects_replaced.push(was);
            return;
        }
        self.object_hash = base.path_hash;
        self.trail.clear();
        self.report.objects_merged.push(base.path_hash);
        self.properties(&mut base.properties, &edited.properties, edited.class_hash);
    }

    fn properties(
        &mut self,
        base: &mut IndexMap<BinHash, PropertyValueEnum<M>>,
        edited: &'e IndexMap<BinHash, PropertyValueEnum<M>>,
        class: BinHash,
    ) {
        for (field, value) in edited {
            match base.get_mut(field) {
                Some(existing) => {
                    self.trail.push_field(*field, class);
                    self.value(existing, value);
                    self.trail.pop();
                }
                None => {
                    base.insert(*field, value.clone());
                    self.report.inserted += 1;
                }
            }
        }
    }

    fn value(&mut self, base: &mut PropertyValueEnum<M>, edited: &'e PropertyValueEnum<M>) {
        use PropertyValueEnum as V;

        match (&mut *base, edited) {
            (V::Struct(b), V::Struct(e))
            | (V::Embedded(values::Embedded(b)), V::Embedded(values::Embedded(e)))
                if combines(b, e) =>
            {
                return self.properties(&mut b.properties, &e.properties, e.class_hash);
            }
            (V::Map(b), V::Map(e))
                if b.key_kind() == e.key_kind() && b.value_kind() == e.value_kind() =>
            {
                if self.map(b, e) {
                    return;
                }
            }
            (V::Optional(b), V::Optional(e)) if b.item_kind() == e.item_kind() => {
                if let (Some(b), Some(e)) = (b.slot(), e.value()) {
                    self.trail.push(TrailSegment::Index(0));
                    self.value(b.into_inner(), e);
                    self.trail.pop();
                    return;
                }
            }
            _ => {}
        }

        if *base != *edited {
            let mismatched = mismatched(base, edited);
            let was = mem::replace(base, edited.clone());
            self.record(was, mismatched);
        }
    }

    /// Combines two maps of the same kinds entry by entry. `false`, leaving `base` untouched,
    /// when a key on either side does not convert to a [`MapKey`].
    fn map(&mut self, base: &mut values::Map<M>, edited: &'e values::Map<M>) -> bool {
        let Some(mut index) = key_index(base) else {
            return false;
        };
        let Some(keys) = edited
            .entries()
            .iter()
            .map(|(key, _)| MapKey::try_from(key).ok())
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };

        for ((key, value), map_key) in edited.entries().iter().zip(keys) {
            let existing = index.get(&map_key).and_then(|at| base.slot(*at));
            match existing {
                Some(existing) => {
                    self.trail.push(TrailSegment::Key(key));
                    self.value(ValueSlot::into_inner(existing), value);
                    self.trail.pop();
                }
                None => {
                    index.insert(map_key, base.entries().len());
                    base.push(key.clone(), value.clone())
                        .expect("an entry of a map with the same kinds fits");
                    self.report.keys_inserted += 1;
                }
            }
        }
        true
    }

    /// Records a replacement at the trail's position.
    fn record(&mut self, was: PropertyValueEnum<M>, mismatched: bool) {
        let at = self
            .trail
            .to_value_path()
            .expect("every key on the trail converted when its map was indexed");
        self.report.replaced.push(Replaced {
            object_hash: self.object_hash,
            at,
            was,
            mismatched,
        });
    }
}
