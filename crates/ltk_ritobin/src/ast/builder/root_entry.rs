use crate::{
    ast::{
        diagnostics::Diagnostic as D,
        node::{
            root::{KnownRoot, Root, RootKind, RootValue},
            roots::Roots,
            TypeExpr,
        },
        RootEntry, Value,
    },
    cst::Kind,
    parse::Span,
    Node, Spanned, SpannedExt,
};

use super::*;

use ltk_hash::BinHash;
use ltk_meta::PropertyKind::{self};

#[derive(Debug, Clone)]
pub struct RawRootProperty {
    pub key: Spanned<RootKind>,
    pub type_expr: Spanned<Option<TypeExpr>>,
    pub value: Option<Value>,
}

impl<'a> Builder<'a> {
    pub(crate) fn build(mut self) -> Ast {
        let mut all = self.resolve_roots();
        let mut roots = Roots::default();

        // `Self::resolve_entry` coerces every root, and this loop checks types only
        for (idx, root) in all.iter_mut().enumerate() {
            if self.check_root_type(root) {
                self.gather_root(root, idx, &mut roots);
            }
        }
        roots.all = all;

        for kind in roots.missing() {
            self.push(D::MissingRootEntry { root_kind: kind }.default_span(Span::empty(0)));
        }
        self.resolve_file_kind(&mut roots);

        Ast {
            roots,
            diagnostics: self.diagnostics,
        }
    }

    /// Resolves every top-level entry of the tree to a [`Root`], in file order.
    fn resolve_roots(&mut self) -> Vec<Root> {
        let cst = self.cst;
        let mut idx = 0;
        cst.root()
            .children
            .get(cst)
            .iter()
            .filter_map(|child| {
                let root = self.resolve_root(child.tree(cst)?, idx)?;
                idx += 1;
                Some(root)
            })
            .collect()
    }

    /// Checks a root's type expression and value against the type its kind expects.
    ///
    /// Returns `false` for a value of another type. A root of another type is not gathered.
    fn check_root_type(&mut self, root: &Root) -> bool {
        let Some(expected_type) = root.name.expected_type() else {
            return true;
        };
        match root.type_expr.value {
            Some(type_expr) if type_expr != expected_type => self.push(
                D::InvalidRootEntryType {
                    root_kind: *root.name,
                    key_span: root.name.span,
                    type_span: root.type_expr.span,
                    got: type_expr.into(),
                    expected: expected_type,
                }
                .unwrap(),
            ),
            Some(_) => {}
            None => self.push(
                D::MissingEntryType {
                    key_span: root.name.span,
                }
                .unwrap(),
            ),
        }

        if let Some(RootValue::Value(value)) = root.value.as_ref() {
            if let Some(got) = value.rito_type().filter(|got| *got != expected_type) {
                self.push(
                    D::TypeMismatch {
                        span: value.span(),
                        expected: expected_type.into(),
                        expected_span: None,
                        got: got.into(),
                    }
                    .unwrap(),
                );
                return false;
            }
        }
        true
    }

    /// Resolves the value of `root`, at `idx`, by its kind and stores it in `roots`.
    ///
    /// A `patches` root is gathered without its records. `Self::resolve_file_kind` resolves them
    /// for a `PTCH` file.
    fn gather_root(&mut self, root: &mut Root, idx: usize, roots: &mut Roots) {
        match *root.name {
            RootKind::Unknown => self.push(
                D::MissingEntryValue {
                    key_span: root.name.span,
                    expected: root
                        .name
                        .expected_type()
                        .map(|t| t.with_span(root.type_expr.span)),
                }
                .unwrap(),
            ),
            RootKind::Version => {
                if let Some(RootValue::Value(Value::U32(v))) = &root.value {
                    let version = KnownRoot {
                        idx,
                        value: v.value,
                    };
                    self.keep_root(&mut roots.version, version);
                }
            }
            RootKind::Type => {
                if let Some(v) = root
                    .value
                    .as_ref()
                    .and_then(|v| v.as_value())
                    .and_then(|v| v.as_string())
                {
                    let file_type = KnownRoot {
                        idx,
                        value: v.parse().unwrap(),
                    };
                    self.keep_root(&mut roots.file_type, file_type);
                }
            }
            RootKind::Linked => {
                if let Some(RootValue::Value(Value::Container { items, .. })) = &root.value {
                    let linked = KnownRoot {
                        idx,
                        value: linked_paths(items),
                    };
                    self.keep_root(&mut roots.linked, linked);
                }
            }
            RootKind::Entries => {
                if resolve_entries(root) {
                    let shadowed = roots.entries.replace(idx);
                    self.report_shadowed(idx, shadowed);
                }
            }
            RootKind::Patches => {
                if let Some(RootValue::Value(Value::Map { .. })) = &root.value {
                    let patches = KnownRoot {
                        idx,
                        value: Vec::new(),
                    };
                    self.keep_root(&mut roots.patches, patches);
                }
            }
            RootKind::Deleted => {
                if let Some(RootValue::Value(Value::Container { items, .. })) = &root.value {
                    let deleted = KnownRoot {
                        idx,
                        value: deleted_hashes(items),
                    };
                    self.keep_root(&mut roots.deleted, deleted);
                }
            }
        }
    }

    /// Stores `root` in `slot`, diagnosing the root it shadows.
    fn keep_root<V>(&mut self, slot: &mut Option<KnownRoot<V>>, root: KnownRoot<V>) {
        let shadower = root.idx;
        let shadowed = slot.replace(root).map(|existing| existing.idx);
        self.report_shadowed(shadower, shadowed);
    }

    fn report_shadowed(&mut self, shadower: usize, shadowee: Option<usize>) {
        if let Some(shadowee) = shadowee {
            self.push(D::ShadowedRoot { shadower, shadowee }.default_span(Span::empty(0)));
        }
    }

    fn resolve_root(&mut self, node: &Node, idx: usize) -> Option<Root> {
        match node.kind {
            Kind::Comment | Kind::ErrorTree => return None,
            Kind::Entry => match self.resolve_entry(node, None, None) {
                Ok(entry) => {
                    let kind = RootKind::from_value(&entry.key);

                    return Some(Root {
                        idx,
                        name: kind.with_span(entry.key.span()),
                        type_expr: entry.type_expr,
                        value: entry.value.map(RootValue::Value),
                    });
                }
                Err(e) => {
                    self.push(e.fallback(node.span));
                }
            },
            _ => {
                self.push(D::RootNonEntry.default_span(node.span));
            }
        }
        None
    }
}

/// The paths of a `linked` root's items. An item that is not a string is left out.
fn linked_paths(items: &[Value]) -> Vec<String> {
    items
        .iter()
        .filter_map(|v| {
            v.clone()
                .try_coerce_to(PropertyKind::String)
                .ok()
                .and_then(|v| v.into_string())
        })
        .collect()
}

/// The object hashes of a `deleted` root's items. An item that is not a hash is left out.
fn deleted_hashes(items: &[Value]) -> Vec<BinHash> {
    items
        .iter()
        .filter_map(|item| match item {
            Value::Hash(hash) => Some(hash.value),
            _ => None,
        })
        .collect()
}

/// Replaces a well-formed `entries` map with its [`RootEntry`] list.
///
/// Returns `false` for any other value, and leaves it in place to navigate and diagnose.
fn resolve_entries(root: &mut Root) -> bool {
    match root.value.take() {
        Some(RootValue::Value(Value::Map {
            entries: map, span, ..
        })) => {
            let items = map
                .into_iter()
                .filter_map(|(k, v)| match (k, v) {
                    (Value::Hash(path_hash), Some(Value::Embedded(object))) => {
                        Some(RootEntry { path_hash, object })
                    }
                    _ => None,
                })
                .collect();
            root.value = Some(RootValue::Entries(Spanned::new(span, items)));
            true
        }
        other => {
            root.value = other;
            false
        }
    }
}
