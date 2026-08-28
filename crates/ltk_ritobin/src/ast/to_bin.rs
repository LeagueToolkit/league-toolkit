use ltk_meta::{
    property::values, traits::PropertyExt as _, Bin, BinObject, Error as MetaError, PropertyKind,
    PropertyValueEnum,
};

use crate::{
    ast::{diagnostics::DiagnosticWithSpan, Ast, Object, RootEntry, Value},
    parse::Span,
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
    pub fn to_bin(&self, text: &str) -> Bin {
        let objects = self
            .objects
            .iter()
            .map(|RootEntry { path_hash, object }| {
                let struct_val = object.to_bin_value().no_meta();
                BinObject {
                    path_hash: path_hash.value,
                    class_hash: struct_val.class_hash,
                    properties: struct_val.properties,
                }
            })
            .collect::<Vec<_>>();

        let dependencies: Vec<String> = self
            .dependencies
            .iter()
            .map(|span| text[Span::new(span.start + 1, span.end - 1)].to_owned())
            .collect();

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
    pub fn to_bin_value(&self) -> values::Struct<Span> {
        values::Struct {
            class_hash: self.class_hash.value,
            properties: self
                .properties
                .iter()
                .filter_map(|p| Some((p.name.value, p.value.as_ref()?.to_bin_value()?)))
                .collect(),
            meta: self.span,
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
    /// Recursively converts this value into an equivalent `PropertyValueEnum<Span>`.
    pub fn to_bin_value(&self) -> Option<PropertyValueEnum<Span>> {
        use PropertyValueEnum as P;
        Some(match self {
            Value::Unknown(_) => return None,
            Value::Unresolved { kind, .. } => kind.default_value(),
            Value::None(v) => P::None(values::None::new(*v)),
            Value::Bool(Spanned { value, span }) => {
                P::Bool(values::Bool::new_with_meta(*value, *span))
            }
            Value::BitBool(Spanned { value, span }) => {
                P::BitBool(values::BitBool::new_with_meta(*value, *span))
            }
            Value::I8(v) => P::I8(v.clone()),
            Value::U8(v) => P::U8(v.clone()),
            Value::I16(v) => P::I16(v.clone()),
            Value::U16(v) => P::U16(v.clone()),
            Value::I32(v) => P::I32(v.clone()),
            Value::U32(v) => P::U32(v.clone()),
            Value::I64(v) => P::I64(v.clone()),
            Value::U64(v) => P::U64(v.clone()),
            Value::F32(v) => P::F32(v.clone()),
            Value::Vector2(v) => P::Vector2(v.clone()),
            Value::Vector3(v) => P::Vector3(v.clone()),
            Value::Vector4(v) => P::Vector4(v.clone()),
            Value::Matrix44(v) => P::Matrix44(v.clone()),
            Value::Color(Spanned { value, span }) => {
                P::Color(values::Color::new_with_meta(*value, *span))
            }
            Value::String(Spanned { value, span }) => {
                P::String(values::String::new_with_meta(value.clone(), *span))
            }
            Value::Hash(v) => P::Hash(values::Hash::new_with_meta(v.value, v.span())),
            Value::WadChunkLink(v) => {
                P::WadChunkLink(values::WadChunkLink::new_with_meta(v.value, v.span()))
            }
            Value::ObjectLink(v) => {
                P::ObjectLink(values::ObjectLink::new_with_meta(v.value, v.span()))
            }
            Value::Struct(s) => P::Struct(s.to_bin_value()),
            Value::Embedded(s) => P::Embedded(values::Embedded(s.to_bin_value())),
            Value::Container {
                item_kind,
                items,
                span,
            } => P::Container(container_from(*item_kind, items, *span)),
            Value::UnorderedContainer {
                item_kind,
                items,
                span,
            } => P::UnorderedContainer(values::UnorderedContainer(container_from(
                *item_kind, items, *span,
            ))),
            Value::Map {
                key_kind,
                value_kind,
                entries,
                span,
            } => {
                let mut map = values::Map::empty(*key_kind, *value_kind);
                for (k, v) in entries {
                    if let Some((k, v)) = k
                        .to_bin_value()
                        .zip(v.as_ref().and_then(|v| v.to_bin_value()))
                    {
                        assert(map.push(k, v), || ());
                    }
                }
                *map.meta_mut() = *span;
                P::Map(map)
            }
            Value::Optional {
                item_kind,
                value,
                span,
            } => {
                let inner = value.as_deref().and_then(Value::to_bin_value);
                let item_kind = (*item_kind)?;
                let optional = assert(
                    values::Optional::new_with_meta(item_kind, inner, *span),
                    || values::Optional::empty(item_kind).unwrap_or_else(|| none_optional(*span)),
                );
                P::Optional(optional)
            }
        })
    }
}

fn container_from(item_kind: PropertyKind, items: &[Value], span: Span) -> values::Container<Span> {
    let mut container = assert(values::Container::empty(item_kind), || {
        values::Container::empty(PropertyKind::None).expect("None is always a valid item kind")
    });
    for item in items {
        if let Some(value) = item.to_bin_value() {
            assert(container.push(value), || ());
        }
    }
    *container.meta_mut() = span;
    container
}

fn none_optional(span: Span) -> values::Optional<Span> {
    let mut optional = values::Optional::empty(PropertyKind::None)
        .expect("None is always a valid item kind for Optional");
    *optional.meta_mut() = span;
    optional
}
