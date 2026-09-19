use ltk_meta::{
    property::values, Bin, BinObject, Error as MetaError, PropertyKind, PropertyValueEnum,
};

use crate::{
    ast::{diagnostics::DiagnosticWithSpan, Ast, Object, RootEntry, Value},
    Spanned,
};

pub struct PartialBin {
    pub bin: Bin,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl PartialBin {
    #[allow(clippy::result_large_err)]
    #[inline(always)]
    pub fn into_result(self) -> Result<Bin, Self> {
        if self.diagnostics.is_empty() {
            Ok(self.bin)
        } else {
            Err(self)
        }
    }
}

impl Ast {
    pub fn to_bin(&self, _text: &str) -> Bin {
        let objects = self
            .root_entries()
            .map(|RootEntry { path_hash, object }| {
                let struct_val = object.to_bin_value();
                BinObject {
                    path_hash: path_hash.value,
                    class_hash: struct_val.class_hash,
                    properties: struct_val.properties,
                }
            })
            .collect::<Vec<_>>();

        let dependencies: Vec<String> = self
            .roots
            .linked
            .clone()
            .map(|linked| linked.into_inner())
            .unwrap_or_default();

        Bin::new(objects, dependencies)
    }

    pub fn into_partial_bin(self, text: &str) -> PartialBin {
        let bin = self.to_bin(text);
        PartialBin {
            bin,
            diagnostics: self.diagnostics,
        }
    }
}

impl Object {
    pub fn to_bin_value(&self) -> values::Struct {
        values::Struct {
            class_hash: self.class_hash.value,
            properties: self
                .properties
                .iter()
                .filter_map(|p| Some((p.name.value, p.value.as_ref()?.to_bin_value()?)))
                .collect(),
        }
    }
}

fn assert<T>(result: Result<T, MetaError>, fallback: impl FnOnce() -> T) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            debug_assert!(false, "ast::build should have prevented this: {e:?}");
            fallback()
        }
    }
}

impl Value {
    /// Recursively converts this value into an equivalent `PropertyValueEnum`.
    pub fn to_bin_value(&self) -> Option<PropertyValueEnum> {
        use PropertyValueEnum as P;
        Some(match self {
            Value::Unknown(_) => return None,
            Value::Unresolved { kind, .. } => kind.default_value(),
            Value::None(_) => P::None(values::None),
            Value::Bool(Spanned { value, .. }) => P::Bool(values::Bool::new(*value)),
            Value::BitBool(Spanned { value, .. }) => P::BitBool(values::BitBool::new(*value)),
            Value::I8(v) => P::I8(v.value.into()),
            Value::U8(v) => P::U8(v.value.into()),
            Value::I16(v) => P::I16(v.value.into()),
            Value::U16(v) => P::U16(v.value.into()),
            Value::I32(v) => P::I32(v.value.into()),
            Value::U32(v) => P::U32(v.value.into()),
            Value::I64(v) => P::I64(v.value.into()),
            Value::U64(v) => P::U64(v.value.into()),
            Value::F32(v) => P::F32(v.value.into()),
            Value::Vector2(v) => P::Vector2(v.value.into()),
            Value::Vector3(v) => P::Vector3(v.value.into()),
            Value::Vector4(v) => P::Vector4(v.value.into()),
            Value::Matrix44(v) => P::Matrix44(v.value.into()),
            Value::Color(Spanned { value, .. }) => P::Color(values::Color::new(*value)),
            Value::String(Spanned { value, .. }) => P::String(values::String::new(value.clone())),
            Value::Hash(v) => P::Hash(values::Hash::new(v.value)),
            Value::WadChunkLink(v) => P::WadChunkLink(values::WadChunkLink::new(v.value)),
            Value::ObjectLink(v) => P::ObjectLink(values::ObjectLink::new(v.value)),
            Value::Struct(s) => P::Struct(s.to_bin_value()),
            Value::Embedded(s) => P::Embedded(values::Embedded(s.to_bin_value())),
            Value::Container {
                item_kind,
                items,
                span: _,
            } => P::Container(container_from(*item_kind, items)),
            Value::UnorderedContainer {
                item_kind,
                items,
                span: _,
            } => P::UnorderedContainer(values::UnorderedContainer(container_from(
                *item_kind, items,
            ))),
            Value::Map {
                key_kind,
                value_kind,
                entries,
                span: _,
            } => {
                let mut map = assert(values::Map::empty(*key_kind, *value_kind), || {
                    values::Map::empty(PropertyKind::None, PropertyKind::None)
                        .expect("None is always a valid map key and value kind")
                });
                for (k, v) in entries {
                    if let Some((k, v)) = k
                        .to_bin_value()
                        .zip(v.as_ref().and_then(|v| v.to_bin_value()))
                    {
                        assert(map.push(k, v), || ());
                    }
                }
                P::Map(map)
            }
            Value::Optional {
                item_kind,
                value,
                span: _,
            } => {
                let inner = value.as_deref().and_then(Value::to_bin_value);
                let item_kind = (*item_kind)?;
                let optional = assert(values::Optional::new(item_kind, inner), || {
                    values::Optional::empty(item_kind).unwrap_or_default()
                });
                P::Optional(optional)
            }
        })
    }
}

fn container_from(item_kind: PropertyKind, items: &[Value]) -> values::Container {
    let mut container = assert(values::Container::empty(item_kind), || {
        values::Container::empty(PropertyKind::None).expect("None is always a valid item kind")
    });
    for item in items {
        if let Some(value) = item.to_bin_value() {
            assert(container.push(value), || ());
        }
    }
    container
}
