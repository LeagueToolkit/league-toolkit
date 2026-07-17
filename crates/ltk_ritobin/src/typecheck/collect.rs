use ltk_meta::{
    property::values, traits::PropertyExt, Bin, BinObject, PropertyKind, PropertyValueEnum,
};

use crate::{
    parse::Span,
    typecheck::diagnostics::{self, RootKind},
    RitoType,
};

use super::{
    resolve::coerce_type,
    state::{RootEntry, RootKindOrUnknown, TypeChecker},
};

use diagnostics::Diagnostic::*;

impl<'a> TypeChecker<'a> {
    /// Pops `entry`'s value out if `extract` succeeds; otherwise pushes an
    /// `InvalidRootEntryType` diagnostic (using `extract`'s returned value to report what was
    /// actually found) and returns `None`. Does not handle the "entry is absent" case - callers
    /// do that themselves before calling this.
    fn take_root_value<T>(
        &mut self,
        root_kind: RootKind,
        entry: RootEntry,
        type_span: Span,
        expected: PropertyKind,
        extract: impl FnOnce(PropertyValueEnum<Span>) -> Result<T, PropertyValueEnum<Span>>,
    ) -> Option<T> {
        let key_span = *entry.key.meta();
        match extract(entry.value) {
            Ok(v) => Some(v),
            Err(got) => {
                self.ctx.diagnostics.push(
                    InvalidRootEntryType {
                        root_kind,
                        key_span,
                        type_span,
                        got: RitoType::simple(got.kind()),
                        expected: RitoType::simple(expected),
                    }
                    .unwrap(),
                );
                None
            }
        }
    }

    pub fn collect_to_bin(mut self) -> (Bin, Vec<diagnostics::DiagnosticWithSpan>) {
        let dependencies = self
            .root
            .swap_remove(&RootKindOrUnknown::Known(RootKind::Linked));

        if dependencies.is_none() {
            self.ctx.diagnostics.push(
                MissingRootEntry {
                    root_kind: RootKind::Linked,
                }
                .default_span(Span::default()),
            );
        }

        let dependencies = dependencies
            .and_then(|e| {
                let type_span = e.type_span;
                self.take_root_value(
                    RootKind::Linked,
                    e,
                    type_span,
                    PropertyKind::Container,
                    |value| match value {
                        PropertyValueEnum::Container(list) => Ok(list),
                        other => Err(other),
                    },
                )
            })
            .map(|list| {
                list.into_items()
                    .filter_map(|value| {
                        let span = *value.meta();
                        let PropertyValueEnum::String(dependency) =
                            coerce_type(value, PropertyKind::String)?
                        else {
                            self.ctx.diagnostics.push(
                                UnexpectedContainerItem {
                                    span,
                                    expected: RitoType::simple(PropertyKind::String),
                                    expected_span: None,
                                }
                                .unwrap(),
                            );
                            return None;
                        };
                        Some(dependency.value)
                    })
                    .collect::<Vec<_>>()
            });

        let objects = self
            .root
            .swap_remove(&RootKindOrUnknown::Known(RootKind::Entries));

        if objects.is_none() {
            self.ctx.diagnostics.push(
                MissingRootEntry {
                    root_kind: RootKind::Entries,
                }
                .default_span(Span::default()),
            );
        }

        let objects = objects
            .and_then(|e| {
                let type_span = *e.key.meta();
                self.take_root_value(
                    RootKind::Entries,
                    e,
                    type_span,
                    PropertyKind::Map,
                    |value| match value {
                        PropertyValueEnum::Map(map) => Ok(map),
                        other => Err(other),
                    },
                )
            })
            .map(|map| {
                map.into_entries()
                    .into_iter()
                    .filter_map(|(key, value)| {
                        let PropertyValueEnum::Hash(path_hash) =
                            coerce_type(key, PropertyKind::Hash)?
                        else {
                            return None;
                        };

                        if let PropertyValueEnum::Embedded(values::Embedded(struct_val)) = value {
                            let struct_val = struct_val.no_meta();
                            Some(BinObject {
                                path_hash: *path_hash,
                                class_hash: struct_val.class_hash,
                                properties: struct_val.properties,
                            })
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
            });

        match self.root.swap_remove(&RootKind::Type) {
            Some(bin_type) => {
                let type_span = *bin_type.key.meta();
                if let Some(type_value) = self.take_root_value(
                    RootKind::Type,
                    bin_type,
                    type_span,
                    PropertyKind::String,
                    |value| match value {
                        PropertyValueEnum::String(s) => Ok(s),
                        other => Err(other),
                    },
                ) {
                    match type_value.as_str() {
                        "PROP" => {}
                        "PTCH" => {
                            self.ctx.diagnostics.push(
                                CustomSpan("Patch bins are not supported yet", *type_value.meta())
                                    .unwrap(),
                            );
                        }
                        _other => {
                            self.ctx
                                .diagnostics
                                .push(CustomSpan("Unknown bin type", *type_value.meta()).unwrap());
                        }
                    }
                }
            }
            None => {
                self.ctx.diagnostics.push(
                    MissingRootEntry {
                        root_kind: RootKind::Type,
                    }
                    .default_span(Span::default()),
                );
            }
        }
        match self.root.swap_remove(&RootKind::Version) {
            Some(version) => {
                let type_span = *version.key.meta();
                if let Some(version_value) = self.take_root_value(
                    RootKind::Version,
                    version,
                    type_span,
                    PropertyKind::U32,
                    |value| match value {
                        PropertyValueEnum::U32(v) => Ok(v),
                        other => Err(other),
                    },
                ) {
                    match *version_value {
                        3 => {}
                        _other => {
                            self.ctx.diagnostics.push(
                                CustomSpan("Bin version should be '3'", *version_value.meta())
                                    .unwrap(),
                            );
                        }
                    }
                }
            }
            None => {
                self.ctx.diagnostics.push(
                    MissingRootEntry {
                        root_kind: RootKind::Version,
                    }
                    .default_span(Span::default()),
                );
            }
        }

        for (_key, unknown) in self.root {
            self.ctx.diagnostics.push(
                UnknownRoot {
                    span: *unknown.key.meta(),
                }
                .default_span(Span::default()),
            );
        }

        let tree = Bin::new(
            objects.unwrap_or_default(),
            dependencies.unwrap_or_default(),
        );

        (tree, self.ctx.diagnostics)
    }
}
