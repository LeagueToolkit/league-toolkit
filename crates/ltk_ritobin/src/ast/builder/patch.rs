use ltk_hash::{BinHash, Hash as _};
use ltk_meta::{path::PropertyPath, PropertyKind};

use crate::{
    ast::{
        diagnostics::{Diagnostic as D, PatchField},
        node::{
            root::{FileKind, RootKind, RootValue},
            roots::Roots,
        },
        Property, RootPatch, Value,
    },
    RitoType, Spanned,
};

use super::Builder;

/// The class of every record embed in the `patches` root.
const PATCH_CLASS: &str = "patch";

/// The `version` root of every `PTCH` text file. It is the inner `PROP` version of the binary.
const PATCH_TEXT_VERSION: u32 = 3;

impl Builder<'_> {
    /// Resolves and checks what the file kind of the `type` root decides.
    ///
    /// A `PTCH` file has its records resolved and its `version` and `linked` roots checked. Any
    /// other file has its `patches` and `deleted` roots diagnosed, and its records unresolved.
    pub(super) fn resolve_file_kind(&mut self, roots: &mut Roots) {
        if roots.file_type().map(|root| root.value) == Some(FileKind::Patch) {
            self.resolve_patch_records(roots);
            self.check_patch_roots(roots);
            return;
        }
        for root in &*roots {
            if matches!(*root.name, RootKind::Patches | RootKind::Deleted) {
                self.push(
                    D::PatchOnlyRoot {
                        span: root.name.span,
                        root_kind: *root.name,
                    }
                    .unwrap(),
                );
            }
        }
    }

    /// Resolves the records of the gathered `patches` root, in the order they are written.
    fn resolve_patch_records(&mut self, roots: &mut Roots) {
        let Some(patches) = roots.patches.as_mut() else {
            return;
        };
        let Some(Some(RootValue::Value(Value::Map { entries, .. }))) =
            roots.all.get(patches.idx).map(|root| &root.value)
        else {
            return;
        };
        patches.value = entries
            .iter()
            .filter_map(|(key, value)| self.resolve_patch(key, value.as_ref()))
            .collect();
    }

    /// Diagnoses the `PTCH` authoring rules on the roots every file has: version 3, no links.
    fn check_patch_roots(&mut self, roots: &Roots) {
        if let Some(version) = roots.version() {
            if version.value != PATCH_TEXT_VERSION {
                self.push(
                    D::UnsupportedPatchVersion {
                        span: version.original(roots).value_span(),
                        version: version.value,
                    }
                    .unwrap(),
                );
            }
        }
        if let Some(linked) = roots.linked() {
            if !linked.value.is_empty() {
                self.push(
                    D::PatchLinked {
                        span: linked.original(roots).value_span(),
                    }
                    .unwrap(),
                );
            }
        }
    }

    /// Resolves one `object = patch { .. }` pair of the `patches` root.
    ///
    /// Returns `None` for a record without a usable path or value. The map resolver diagnoses a
    /// key that is not a hash and a value that is not an embed; this diagnoses the rest.
    fn resolve_patch(&mut self, key: &Value, value: Option<&Value>) -> Option<RootPatch> {
        let Value::Hash(object_hash) = key else {
            return None;
        };
        let Some(Value::Embedded(record)) = value else {
            return None;
        };

        if record.class_hash.value != BinHash::hash_str(PATCH_CLASS) {
            self.push(
                D::UnexpectedPatchClass {
                    span: record.class_hash.span(),
                }
                .unwrap(),
            );
        }

        let mut fields = PatchFields::default();
        for property in &record.properties {
            let Some(field) = PatchField::named(property.name.value) else {
                self.push(
                    D::UnexpectedPatchField {
                        span: property.span(),
                    }
                    .unwrap(),
                );
                continue;
            };
            let slot = fields.slot(field);
            if slot.is_some() {
                self.push(
                    D::DuplicatePatchField {
                        span: property.name.span(),
                        field,
                    }
                    .unwrap(),
                );
                continue;
            }
            *slot = Some(property);
        }
        for field in PatchField::ALL {
            if fields.get(field).is_none() {
                self.push(
                    D::MissingPatchField {
                        span: record.class_hash.span(),
                        field,
                    }
                    .unwrap(),
                );
            }
        }

        let path = self.resolve_patch_path(fields.path?.value.as_ref());
        let patch_value = fields.value?.value.as_ref().filter(|v| is_resolved(v));
        Some(RootPatch {
            object_hash: *object_hash,
            path: path?,
            value: patch_value?.clone(),
            span: object_hash.span().cover(record.span),
        })
    }

    /// Validates the value of a record's `path` field as a [`PropertyPath`].
    fn resolve_patch_path(&mut self, value: Option<&Value>) -> Option<Spanned<PropertyPath>> {
        match value? {
            Value::String(text) => match PropertyPath::new(text.value.as_str()) {
                Ok(path) => Some(Spanned::new(text.span, path)),
                Err(error) => {
                    self.push(
                        D::InvalidPropertyPath {
                            span: text.span,
                            error,
                        }
                        .unwrap(),
                    );
                    None
                }
            },
            // an unresolved value carries its own diagnostic
            value if !is_resolved(value) => None,
            value => {
                self.push(
                    D::TypeMismatch {
                        span: value.span(),
                        expected: RitoType::simple(PropertyKind::String).into(),
                        expected_span: None,
                        got: value.rito_type().into(),
                    }
                    .unwrap(),
                );
                None
            }
        }
    }
}

/// The first `path` and `value` field of one `patch` embed.
#[derive(Default)]
struct PatchFields<'a> {
    path: Option<&'a Property>,
    value: Option<&'a Property>,
}

impl<'a> PatchFields<'a> {
    fn get(&self, field: PatchField) -> Option<&'a Property> {
        match field {
            PatchField::Path => self.path,
            PatchField::Value => self.value,
        }
    }

    fn slot(&mut self, field: PatchField) -> &mut Option<&'a Property> {
        match field {
            PatchField::Path => &mut self.path,
            PatchField::Value => &mut self.value,
        }
    }
}

/// Whether `value` resolved to a value of its own. An unresolved value carries the diagnostic of
/// the resolution that failed.
fn is_resolved(value: &Value) -> bool {
    !matches!(value, Value::Unresolved { .. } | Value::Unknown(_))
}
