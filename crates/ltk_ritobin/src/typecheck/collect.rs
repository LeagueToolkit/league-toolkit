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
    state::{RootKindOrUnknown, TypeChecker},
};

use diagnostics::Diagnostic::*;

impl<'a> TypeChecker<'a> {
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

        let dependencies = dependencies.and_then(|e| {
            let PropertyValueEnum::Container(list) = e.value else {
                self.ctx.diagnostics.push(
                    InvalidRootEntryType {
                        root_kind: RootKind::Linked,

                        key_span: *e.key.meta(),
                        type_span: e.type_span,

                        expected: RitoType::simple(PropertyKind::Container),
                        got: RitoType::simple(e.value.kind()),
                    }
                    .unwrap(),
                );
                return None;
            };

            Some(
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
                    .collect::<Vec<_>>(),
            )
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

        let objects = objects.and_then(|e| {
            let PropertyValueEnum::Map(map) = e.value else {
                self.ctx.diagnostics.push(
                    InvalidRootEntryType {
                        root_kind: RootKind::Entries,
                        key_span: *e.key.meta(),
                        type_span: *e.key.meta(),
                        got: RitoType::simple(e.value.kind()),
                        expected: RitoType::simple(PropertyKind::Map),
                    }
                    .unwrap(),
                );
                return None;
            };
            Some(
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
                    .collect::<Vec<_>>(),
            )
        });

        match self.root.swap_remove(&RootKind::Type) {
            Some(bin_type) => {
                if let PropertyValueEnum::String(type_value) = bin_type.value {
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
                } else {
                    self.ctx.diagnostics.push(
                        InvalidRootEntryType {
                            root_kind: RootKind::Type,
                            key_span: *bin_type.key.meta(),
                            type_span: *bin_type.key.meta(),
                            got: RitoType::simple(bin_type.value.kind()),
                            expected: RitoType::simple(PropertyKind::String),
                        }
                        .unwrap(),
                    );
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
                if let PropertyValueEnum::U32(version) = version.value {
                    match *version {
                        3 => {}
                        _other => {
                            self.ctx.diagnostics.push(
                                CustomSpan("Bin version should be '3'", *version.meta()).unwrap(),
                            );
                        }
                    }
                } else {
                    self.ctx.diagnostics.push(
                        InvalidRootEntryType {
                            root_kind: RootKind::Version,
                            key_span: *version.key.meta(),
                            type_span: *version.key.meta(),
                            got: RitoType::simple(version.value.kind()),
                            expected: RitoType::simple(PropertyKind::U32),
                        }
                        .unwrap(),
                    );
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
