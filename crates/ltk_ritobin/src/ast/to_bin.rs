use ltk_meta::{property::values, traits::PropertyExt as _, Bin, BinObject, PropertyValueEnum};

use crate::{
    ast::{
        build::{Ast, AstObject},
        AstStruct, AstValue,
    },
    parse::Span,
};

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
}

impl AstValue {
    /// Recursively converts this value into an equivalent `PropertyValueEnum<Span>`.
    ///
    /// **NOTE:** this conversion quietly ignores/skips container related errors (pushing entries/items with invalid types)
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
            } => {
                let items = items.iter().map(AstValue::to_bin_value).collect::<Vec<_>>();
                let container = values::Container::try_from(items).unwrap_or_else(|_| {
                    values::Container::empty(*item_kind).unwrap_or(values::Container::None {
                        items: Vec::new(),
                        meta: *span,
                    })
                });
                PropertyValueEnum::Container(container)
            }
            AstValue::UnorderedContainer {
                item_kind,
                items,
                span,
            } => {
                let items = items.iter().map(AstValue::to_bin_value).collect::<Vec<_>>();
                let container = values::Container::try_from(items).unwrap_or_else(|_| {
                    values::Container::empty(*item_kind).unwrap_or(values::Container::None {
                        items: Vec::new(),
                        meta: *span,
                    })
                });
                PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container))
            }
            AstValue::Map {
                key_kind,
                value_kind,
                entries,
                span,
            } => {
                let mut map = values::Map::empty(*key_kind, *value_kind);
                for (k, v) in entries {
                    let _ = map.push(k.to_bin_value(), v.to_bin_value());
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
                let optional = values::Optional::new_with_meta(*item_kind, inner, *span)
                    .unwrap_or_else(|_| {
                        values::Optional::empty(*item_kind).unwrap_or(values::Optional::None {
                            value: None,
                            meta: *span,
                        })
                    });
                PropertyValueEnum::Optional(optional)
            }
        }
    }
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
