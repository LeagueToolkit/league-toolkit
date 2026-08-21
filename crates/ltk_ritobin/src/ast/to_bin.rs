//! Converts a resolved [`Ast`] into a plain [`Bin`] - the direct counterpart to
//! [`crate::typecheck::TypeChecker::collect_to_bin`], just starting from [`AstValue`] instead of
//! merging into `ltk_meta` types while walking.

use ltk_meta::{property::values, traits::PropertyExt as _, Bin, BinObject, PropertyValueEnum};

use crate::{
    ast::{
        build::{Ast, AstObject},
        nodes::{AstStruct, AstValue},
    },
    parse::Span,
};

impl Ast {
    /// Builds a plain [`Bin`] from this tree. Non-consuming: a caller can hold onto `Ast` (e.g.
    /// for hover/lints) and still export a `Bin` on demand. `text` is what turns
    /// `dependencies`' spans back into the owned `String`s `Bin` needs - passed explicitly
    /// rather than stored, same convention as `Cst::build_bin`/`Cst::print` already use.
    pub fn to_bin(&self, text: &str) -> Bin {
        let objects = self
            .objects
            .iter()
            .map(|AstObject { path_hash, object }| {
                let struct_val = object.to_spanned().no_meta();
                BinObject {
                    path_hash: path_hash.value,
                    class_hash: struct_val.class_hash,
                    properties: struct_val.properties,
                }
            })
            .collect::<Vec<_>>();

        // `dependencies` spans cover the whole string literal, quotes included (same convention
        // as `ast::Literal::String`) - strip them to get the dependency path itself.
        let dependencies: Vec<String> = self
            .dependencies
            .iter()
            .map(|span| text[Span::new(span.start + 1, span.end - 1)].to_owned())
            .collect();

        Bin::new(objects, dependencies)
    }
}

impl AstValue {
    /// Recursively converts this value into an equivalent `PropertyValueEnum<Span>` - the bridge
    /// back to `ltk_meta`'s own container constructors (`Container::TryFrom`, `Map::new`, ...),
    /// which know how to assemble the binary-format-shaped types this crate ultimately needs.
    fn to_spanned(&self) -> PropertyValueEnum<Span> {
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
            AstValue::Struct(s) => PropertyValueEnum::Struct(s.to_spanned()),
            AstValue::Embedded(s) => PropertyValueEnum::Embedded(values::Embedded(s.to_spanned())),
            AstValue::Container {
                item_kind,
                items,
                span,
            } => {
                let items = items.iter().map(AstValue::to_spanned).collect::<Vec<_>>();
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
                let items = items.iter().map(AstValue::to_spanned).collect::<Vec<_>>();
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
                    let _ = map.push(k.to_spanned(), v.to_spanned());
                }
                *map.meta_mut() = *span;
                PropertyValueEnum::Map(map)
            }
            AstValue::Optional {
                item_kind,
                value,
                span,
            } => {
                let inner = value.as_deref().map(AstValue::to_spanned);
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
    fn to_spanned(&self) -> values::Struct<Span> {
        values::Struct {
            class_hash: self.class_hash.value,
            properties: self
                .properties
                .iter()
                .map(|p| (p.name.value, p.value.to_spanned()))
                .collect(),
            meta: self.span,
        }
    }
}
