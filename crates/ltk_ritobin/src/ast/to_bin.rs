use ltk_meta::{
    property::values, traits::PropertyExt as _, Bin, BinObject, Error as MetaError, PropertyKind,
    PropertyValueEnum,
};

use crate::{
    ast::{
        build::{Ast, AstObject},
        diagnostics::DiagnosticWithSpan,
        AstStruct, AstValue,
    },
    parse::Span,
};

pub struct PartialBin {
    pub bin: Bin,
    pub diagnostics: Vec<DiagnosticWithSpan>,
}

impl PartialBin {
    #[allow(clippy::result_large_err)]
    pub fn finish(self) -> Result<Bin, Self> {
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
            .map(|AstObject { path_hash, object }| {
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

fn trust<T>(result: Result<T, MetaError>, fallback: impl FnOnce() -> T) -> T {
    match result {
        Ok(v) => v,
        Err(e) => {
            debug_assert!(false, "ast::build should have prevented this: {e:?}");
            fallback()
        }
    }
}

impl AstValue {
    /// Recursively converts this value into an equivalent `PropertyValueEnum<Span>`.
    pub fn to_bin_value(&self) -> PropertyValueEnum<Span> {
        match self {
            AstValue::None(v) => PropertyValueEnum::None(*v),
            AstValue::Bool(v) => PropertyValueEnum::Bool(v.clone()),
            AstValue::BitBool(v) => PropertyValueEnum::BitBool(v.clone()),
            AstValue::I8(v) => PropertyValueEnum::I8(v.clone()),
            AstValue::U8(v) => PropertyValueEnum::U8(v.clone()),
            AstValue::I16(v) => PropertyValueEnum::I16(v.clone()),
            AstValue::U16(v) => PropertyValueEnum::U16(v.clone()),
            AstValue::I32(v) => PropertyValueEnum::I32(v.clone()),
            AstValue::U32(v) => PropertyValueEnum::U32(v.clone()),
            AstValue::I64(v) => PropertyValueEnum::I64(v.clone()),
            AstValue::U64(v) => PropertyValueEnum::U64(v.clone()),
            AstValue::F32(v) => PropertyValueEnum::F32(v.clone()),
            AstValue::Vector2(v) => PropertyValueEnum::Vector2(v.clone()),
            AstValue::Vector3(v) => PropertyValueEnum::Vector3(v.clone()),
            AstValue::Vector4(v) => PropertyValueEnum::Vector4(v.clone()),
            AstValue::Matrix44(v) => PropertyValueEnum::Matrix44(v.clone()),
            AstValue::Color(v) => PropertyValueEnum::Color(v.clone()),
            AstValue::String(v) => PropertyValueEnum::String(v.clone()),
            AstValue::Hash(v) => PropertyValueEnum::Hash(v.clone()),
            AstValue::WadChunkLink(v) => PropertyValueEnum::WadChunkLink(v.clone()),
            AstValue::ObjectLink(v) => PropertyValueEnum::ObjectLink(v.clone()),
            AstValue::Struct(s) => PropertyValueEnum::Struct(s.to_bin_value()),
            AstValue::Embedded(s) => {
                PropertyValueEnum::Embedded(values::Embedded(s.to_bin_value()))
            }
            AstValue::Container {
                item_kind,
                items,
                span,
            } => PropertyValueEnum::Container(container_from(*item_kind, items, *span)),
            AstValue::UnorderedContainer {
                item_kind,
                items,
                span,
            } => PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container_from(
                *item_kind, items, *span,
            ))),
            AstValue::Map {
                key_kind,
                value_kind,
                entries,
                span,
            } => {
                let mut map = values::Map::empty(*key_kind, *value_kind);
                for (k, v) in entries {
                    let (key, value) = (k.to_bin_value(), v.to_bin_value());
                    trust(map.push(key, value), || ());
                }
                *map.meta_mut() = *span;
                PropertyValueEnum::Map(map)
            }
            AstValue::Optional {
                item_kind,
                value,
                span,
            } => {
                let inner = value.as_deref().map(AstValue::to_bin_value);
                let optional = trust(
                    values::Optional::new_with_meta(*item_kind, inner, *span),
                    || values::Optional::empty(*item_kind).unwrap_or_else(|| none_optional(*span)),
                );
                PropertyValueEnum::Optional(optional)
            }
        }
    }
}

fn container_from(
    item_kind: PropertyKind,
    items: &[AstValue],
    span: Span,
) -> values::Container<Span> {
    let mut container = trust(values::Container::empty(item_kind), || {
        values::Container::empty(PropertyKind::None).expect("None is always a valid item kind")
    });
    for item in items {
        let value = item.to_bin_value();
        trust(container.push(value), || ());
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

impl AstStruct {
    pub fn to_bin_value(&self) -> values::Struct<Span> {
        values::Struct {
            class_hash: self.class_hash.value,
            properties: self
                .properties
                .iter()
                .map(|p| (p.name.value, p.value.to_bin_value()))
                .collect(),
            meta: self.span,
        }
    }
}
