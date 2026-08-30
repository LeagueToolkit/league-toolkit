//! Laying a [`BinOverride`] over the [`Bin`] it patches.

use std::fmt;

use ltk_hash::BinHash;

use crate::{
    path::{PatchError, PropertyPath, ResolveError, ResolveErrorKind},
    Bin, BinObject, BinOverride, PropertyPatch,
};

/// What laying a [`BinOverride`] over a [`Bin`] did, or would do.
///
/// Nothing in a patch is fatal. The client skips a record it cannot apply and carries on loading,
/// so [`BinOverride::apply`] does the same and reports what it skipped instead of failing.
///
/// # Examples
///
/// ```no_run
/// use std::fs::File;
/// use ltk_meta::{Bin, BinOverride};
///
/// let mut base = Bin::from_reader(&mut File::open("uibase.bin")?)?;
/// let patch_bin = BinOverride::from_reader(&mut File::open("uiflipped.bin")?)?;
///
/// let report = patch_bin.apply(&mut base);
/// println!("{report}");
///
/// for skipped in &report.skipped {
///     println!("record {}: {} {} - {}", skipped.index, skipped.object_hash, skipped.path, skipped.error);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Objects the delete list actually removed from the base.
    pub deleted: Vec<BinHash>,
    /// Objects the patch added to the base.
    pub added: Vec<BinHash>,
    /// Objects the patch replaced, because the base already had that hash.
    pub replaced: Vec<BinHash>,
    /// Records applied. For [`BinOverride::check`], records that would apply.
    pub applied: usize,
    /// Of those, records whose leaf did not exist and was created.
    pub inserted: usize,
    /// Records that could not be applied, in file order.
    pub skipped: Vec<SkippedPatch>,
}

impl ApplyReport {
    /// Whether every record applied.
    #[must_use]
    #[inline]
    pub fn is_clean(&self) -> bool {
        self.skipped.is_empty()
    }
}

impl fmt::Display for ApplyReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} applied ({} inserted), {} skipped, {} deleted, {} added, {} replaced",
            self.applied,
            self.inserted,
            self.skipped.len(),
            self.deleted.len(),
            self.added.len(),
            self.replaced.len(),
        )
    }
}

/// One record that did not apply.
///
/// Self-contained, because [`BinOverride::apply`] has consumed the patch by the time the report
/// is read.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedPatch {
    /// Where the record sat in the patch, counting from 0.
    pub index: usize,
    /// The object the record addressed.
    pub object_hash: BinHash,
    /// The property the record addressed.
    pub path: PropertyPath,
    /// Why it did not apply.
    pub error: PatchError,
}

impl fmt::Display for SkippedPatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "record {} ({:08x} {}): {}",
            self.index, self.object_hash, self.path, self.error
        )
    }
}

fn missing_object(object_hash: BinHash) -> PatchError {
    ResolveError::new(0, ResolveErrorKind::MissingObject(object_hash)).into()
}

impl<M> BinOverride<M> {
    /// Lays this patch over `base`, in the order the client does.
    ///
    /// 1. Every hash in [`BinOverride::deleted`] is dropped from the base.
    /// 2. This patch's own objects go in, except any the delete list names. One whose hash the
    ///    base already has replaces it. The client would instead carry both in its merged table
    ///    and let a binary search pick one, which an [`indexmap::IndexMap`] cannot represent; no
    ///    shipped patch collides with its base either way.
    /// 3. The records apply in file order, against the merged table, so a record can address an
    ///    object this patch just added.
    ///
    /// A record that does not apply is recorded in [`ApplyReport::skipped`] and the rest carry
    /// on, because that is what the client does with a stale path.
    ///
    /// This consumes the patch: its objects and values move into `base` and nothing is cloned.
    /// To lay one patch over several bases, clone it.
    pub fn apply(self, base: &mut Bin<M>) -> ApplyReport {
        let Self {
            deleted,
            objects,
            patches,
        } = self;
        let mut report = ApplyReport::default();

        for object_hash in &deleted {
            if base.objects.shift_remove(object_hash).is_some() {
                report.deleted.push(*object_hash);
            }
        }

        for (object_hash, object) in objects {
            if deleted.contains(&object_hash) {
                continue;
            }
            match base.objects.insert(object_hash, object) {
                Some(_) => report.replaced.push(object_hash),
                None => report.added.push(object_hash),
            }
        }

        for (index, patch) in patches.into_iter().enumerate() {
            let PropertyPatch {
                object_hash,
                path,
                value,
            } = patch;

            let outcome = match base.objects.get_mut(&object_hash) {
                Some(object) => object.patch(&path, value),
                None => Err(missing_object(object_hash)),
            };

            match outcome {
                Ok(replaced) => {
                    report.applied += 1;
                    report.inserted += usize::from(replaced.is_none());
                }
                Err(error) => report.skipped.push(SkippedPatch {
                    index,
                    object_hash,
                    path,
                    error,
                }),
            }
        }

        report
    }

    /// The same walk as [`BinOverride::apply`], without changing anything.
    ///
    /// It answers "does this patch still fit this bin", which is the question to ask after a game
    /// update. [`ApplyReport::applied`] counts the records that would apply and
    /// [`ApplyReport::skipped`] names the ones that would not.
    ///
    /// One difference from `apply`: records are checked independently, each against `base` as it
    /// stands. `apply` runs them in order, so a record that only fits because an earlier record in
    /// the same patch replaced a pointer or an embed above it is judged here against the value
    /// that earlier record would have overwritten.
    pub fn check(&self, base: &Bin<M>) -> ApplyReport {
        let mut report = ApplyReport::default();

        for object_hash in &self.deleted {
            if base.objects.contains_key(object_hash) {
                report.deleted.push(*object_hash);
            }
        }

        for object_hash in self.objects.keys() {
            if self.deleted.contains(object_hash) {
                continue;
            }
            match base.objects.contains_key(object_hash) {
                true => report.replaced.push(*object_hash),
                false => report.added.push(*object_hash),
            }
        }

        for (index, patch) in self.patches.iter().enumerate() {
            let outcome = match self.merged_object(base, patch.object_hash) {
                Some(object) => object.probe(&patch.path, &patch.value),
                None => Err(missing_object(patch.object_hash)),
            };

            match outcome {
                Ok(inserted) => {
                    report.applied += 1;
                    report.inserted += usize::from(inserted);
                }
                Err(error) => report.skipped.push(SkippedPatch {
                    index,
                    object_hash: patch.object_hash,
                    path: patch.path.clone(),
                    error,
                }),
            }
        }

        report
    }

    /// The object a record would find in the table [`BinOverride::apply`] builds.
    fn merged_object<'a>(
        &'a self,
        base: &'a Bin<M>,
        object_hash: BinHash,
    ) -> Option<&'a BinObject<M>> {
        if self.deleted.contains(&object_hash) {
            return None;
        }

        self.objects
            .get(&object_hash)
            .or_else(|| base.objects.get(&object_hash))
    }
}
