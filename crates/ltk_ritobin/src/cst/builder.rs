use std::{
    fmt::{LowerHex, Write},
    iter::once,
};

use bumpalo::{
    collections::{self, CollectIn},
    Bump,
};
use ltk_hash::BinHash;
use ltk_meta::{property::values, Bin, BinObject, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{Child, Cst, Kind, Node, NodeId},
    parse::{Span, Token, TokenKind as Tok},
    typecheck::visitor::{PropertyValueExt, RitoType},
    HashProvider, RitobinName as _,
};

pub struct Builder<'arena, H: HashProvider> {
    buf: String,
    hashes: H,
    arena: &'arena Bump,
    nodes: collections::Vec<'arena, Node<'arena>>,
}

pub fn token(kind: Tok) -> Child {
    Child::Token(Token {
        kind,
        span: Span::default(),
    })
}
impl<'arena> Builder<'arena, ()> {
    pub fn new(arena: &'arena Bump) -> Builder<'arena, ()> {
        Builder {
            buf: String::new(),
            hashes: (),
            arena,
            nodes: collections::Vec::new_in(arena),
        }
    }
}

impl<'arena, H: HashProvider> Builder<'arena, H> {
    pub fn with_hashes<H2: HashProvider>(self, hashes: H2) -> Builder<'arena, H2> {
        Builder {
            buf: self.buf,
            hashes,
            arena: self.arena,
            nodes: self.nodes,
        }
    }

    pub fn build(mut self, bin: &'arena Bin) -> (Cst<'arena>, String) {
        self.bin_to_cst(bin);
        (Cst { nodes: self.nodes }, self.buf)
    }

    /// Get a reference to the underlying text buffer all [`Cst`]'s built by this builder reference in
    /// their spans.
    pub fn text_buffer(&self) -> &str {
        &self.buf
    }
    /// Get the underlying text buffer all [`Cst`]'s built by this builder reference in
    /// their spans.
    pub fn into_text_buffer(self) -> String {
        self.buf
    }
}

impl<'arena, H: HashProvider> Builder<'arena, H> {
    pub fn tree(&mut self, kind: Kind, children: impl IntoIterator<Item = Child>) -> Child {
        let id = NodeId(self.nodes.len() as u32);

        self.nodes.push(Node {
            span: Span::default(),
            kind,
            children: children.into_iter().collect_in(self.arena),
            errors: collections::Vec::new_in(self.arena),
        });

        Child::Tree(id)
    }

    fn number(&mut self, v: impl std::fmt::Display) -> Child {
        let lit = self.spanned_display(Tok::Number, v);
        self.tree(Kind::Literal, [lit])
    }

    fn spanned_token(&mut self, kind: Tok, str: impl AsRef<str>) -> Child {
        let start = self.buf.len() as u32;
        self.buf.write_str(str.as_ref()).unwrap();
        let end = self.buf.len() as u32;
        Child::Token(Token {
            kind,
            span: Span::new(start, end),
        })
    }

    fn spanned_display(&mut self, kind: Tok, v: impl std::fmt::Display) -> Child {
        let start = self.buf.len() as u32;
        write!(self.buf, "{v}").unwrap();
        let end = self.buf.len() as u32;
        Child::Token(Token {
            kind,
            span: Span::new(start, end),
        })
    }

    fn spanned_hexlit<T: LowerHex>(&mut self, v: T) -> Child {
        let start = self.buf.len() as u32;
        write!(self.buf, "0x{v:x}").unwrap();
        let end = self.buf.len() as u32;
        Child::Token(Token {
            kind: Tok::HexLit,
            span: Span::new(start, end),
        })
    }

    fn string(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::String, v)
    }

    fn hash_hash_lit(&mut self, h: BinHash) -> Child {
        match self.hashes.lookup_hash(h).map(|h| format!("\"{h}\"")) {
            Some(h) => self.spanned_token(Tok::String, h),
            None => self.spanned_hexlit(h),
        }
    }
    fn hash_type_lit(&mut self, h: BinHash) -> Child {
        match self.hashes.lookup_type(h).map(|h| h.to_string()) {
            Some(h) => self.spanned_token(Tok::Name, h),
            None => self.spanned_hexlit(h),
        }
    }
    fn hash_field_lit(&mut self, h: BinHash) -> Child {
        match self.hashes.lookup_field(h).map(|h| h.to_string()) {
            Some(h) => self.spanned_token(Tok::Name, h),
            None => self.spanned_hexlit(h),
        }
    }
    fn hash_entry_lit(&mut self, h: BinHash) -> Child {
        match self.hashes.lookup_entry(h).map(|h| format!("\"{h}\"")) {
            Some(h) => self.spanned_token(Tok::String, h),
            None => self.spanned_hexlit(h),
        }
    }

    fn block(&mut self, children: Vec<Child>) -> Child {
        self.tree(
            Kind::Block,
            once(token(Tok::LCurly))
                .chain(children)
                .chain(once(token(Tok::RCurly))),
        )
    }

    fn bool(&mut self, v: bool) -> Child {
        self.tree(
            Kind::Literal,
            vec![token(match v {
                true => Tok::True,
                false => Tok::False,
            })],
        )
    }

    fn value_to_cst<M: Clone>(&mut self, value: &PropertyValueEnum<M>) -> Child {
        match value {
            PropertyValueEnum::Bool(b) => self.bool(**b),
            PropertyValueEnum::BitBool(b) => self.bool(**b),

            PropertyValueEnum::U8(n) => self.number(**n),
            PropertyValueEnum::U16(n) => self.number(**n),
            PropertyValueEnum::U32(n) => self.number(**n),
            PropertyValueEnum::U64(n) => self.number(**n),
            PropertyValueEnum::I8(n) => self.number(**n),
            PropertyValueEnum::I16(n) => self.number(**n),
            PropertyValueEnum::I32(n) => self.number(**n),
            PropertyValueEnum::I64(n) => self.number(**n),
            PropertyValueEnum::F32(n) => self.number(**n),
            PropertyValueEnum::Vector2(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| {
                        let v = self.number(*v);
                        self.tree(Kind::ListItem, [v])
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Vector3(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| {
                        let v = self.number(*v);
                        self.tree(Kind::ListItem, [v])
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Vector4(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| {
                        let v = self.number(*v);
                        self.tree(Kind::ListItem, [v])
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Matrix44(v) => {
                let items = v
                    .transpose() // ritobin text stores matrices row-major, glam::Mat4 is column-major.
                    .to_cols_array_2d()
                    .iter()
                    .flat_map(|v| {
                        let values = [
                            self.number(v[0]),
                            self.number(v[1]),
                            self.number(v[2]),
                            self.number(v[3]),
                        ];
                        [
                            self.tree(Kind::ListItem, [values[0]]),
                            self.tree(Kind::ListItem, [values[1]]),
                            self.tree(Kind::ListItem, [values[2]]),
                            self.tree(Kind::ListItem, [values[3]]),
                        ]
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Color(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| {
                        let v = self.number(*v);
                        self.tree(Kind::ListItem, [v])
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::String(s) => {
                let s = self.string(&**s);
                self.tree(Kind::Literal, [token(Tok::Quote), s, token(Tok::Quote)])
            }

            // hash/hash-likes
            PropertyValueEnum::Hash(h) => self.hash_hash_lit(**h),
            PropertyValueEnum::WadChunkLink(h) => self.spanned_hexlit(**h),
            PropertyValueEnum::ObjectLink(h) => self.hash_hash_lit(**h),

            PropertyValueEnum::Container(container)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container)) => {
                let mut children = vec![token(Tok::LCurly)];

                for item in container.clone().into_items() {
                    let item = self.value_to_cst(&item);
                    children.push(self.tree(Kind::ListItem, [item]));
                }

                children.push(token(Tok::RCurly));
                self.tree(Kind::TypeArgList, children)
            }
            PropertyValueEnum::Embedded(values::Embedded(s)) | PropertyValueEnum::Struct(s) => {
                let k = self.hash_type_lit(s.class_hash);
                let children = s
                    .properties
                    .iter()
                    .map(|(k, v)| self.property_to_cst(*k, v))
                    .collect();
                let children = self.block(children);
                self.tree(Kind::Class, [k, children])
            }

            PropertyValueEnum::Optional(optional) => {
                let children = match optional.clone().into_inner() {
                    Some(v) => vec![self.value_to_cst(&v)],
                    None => vec![],
                };
                self.block(children)
            }
            PropertyValueEnum::None(_) => self.tree(Kind::Literal, vec![token(Tok::Null)]),

            PropertyValueEnum::Map(map) => {
                let children = map
                    .entries()
                    .iter()
                    .map(|(k, v)| {
                        let k = self.value_to_cst(k);
                        let v = self.value_to_cst(v);
                        self.tree(Kind::Entry, [k, token(Tok::Eq), v])
                    })
                    .collect();
                self.block(children)
            }
        }
    }

    fn entry_tree(&mut self, key: Child, kind: Option<Child>, value: Child) -> Child {
        self.tree(
            Kind::Entry,
            match kind {
                Some(kind) => vec![key, token(Tok::Colon), kind, token(Tok::Eq), value],
                None => vec![key, token(Tok::Eq), value],
            },
        )
    }

    fn rito_type(&mut self, rito_type: RitoType) -> Child {
        let mut children = vec![self.spanned_token(Tok::Name, rito_type.base.to_rito_name())];

        if let Some(sub) = rito_type.subtypes[0] {
            let name = self.spanned_token(Tok::Name, sub.to_rito_name());
            let mut args = vec![token(Tok::LBrack), self.tree(Kind::TypeArg, [name])];
            if let Some(sub) = rito_type.subtypes[1] {
                let name = self.spanned_token(Tok::Name, sub.to_rito_name());
                args.push(self.tree(Kind::TypeArg, [name]));
            }
            args.push(token(Tok::RBrack));
            children.push(self.tree(Kind::TypeArgList, args));
        }

        self.tree(Kind::TypeExpr, children)
    }
    fn property_to_cst<M: Clone>(
        &mut self,
        name_hash: BinHash,
        value: &PropertyValueEnum<M>,
    ) -> Child {
        let k = self.hash_field_lit(name_hash);
        let t = self.rito_type(value.rito_type());
        let v = self.value_to_cst(value);
        self.entry_tree(k, Some(t), v)
    }

    fn class(&mut self, class_name: Child, items: Vec<Child>) -> Child {
        let items = self.block(items);
        self.tree(Kind::Class, [class_name, items])
    }

    fn bin_object_to_cst(&mut self, obj: &BinObject) -> Child {
        let k = self.hash_entry_lit(obj.path_hash);

        let class_hash = self.hash_type_lit(obj.class_hash);
        let class_values = obj
            .properties
            .iter()
            .map(|(k, v)| {
                let k = self.hash_field_lit(*k);
                let k = self.tree(Kind::EntryKey, [k]);

                let t = self.rito_type(v.rito_type());
                let v = self.value_to_cst(v);
                self.entry_tree(k, Some(t), v)
            })
            .collect();

        let value = self.class(class_hash, class_values);
        self.entry_tree(k, None, value)
    }

    fn entry(&mut self, key: impl AsRef<str>, kind: RitoType, value: Child) -> Child {
        let key = self.spanned_token(Tok::Comment, key);
        let kind = self.rito_type(kind);

        let key = self.tree(Kind::EntryKey, [key]);
        let value = self.tree(Kind::EntryValue, [value]);

        self.entry_tree(key, Some(kind), value)
    }

    fn bin_to_cst(&mut self, bin: &'arena Bin) {
        let root = Node {
            kind: Kind::File,
            span: Span::default(),
            children: collections::Vec::new_in(self.arena),
            errors: collections::Vec::new_in(self.arena),
        };
        self.nodes.push(root);

        let comment = self.spanned_token(Tok::Comment, "#PROP_text");
        let comment = self.tree(Kind::Comment, vec![comment]);

        let type_entry = self.string("\"PROP\"");
        let type_entry = self.entry("type", RitoType::simple(PropertyKind::String), type_entry);

        let version = self.number("3");
        let version = self.entry("version", RitoType::simple(PropertyKind::U32), version);

        let linked = bin
            .dependencies
            .iter()
            .map(|dep| {
                let lit = self.string(format!("\"{dep}\""));
                let lit = self.tree(Kind::Literal, [lit]);
                self.tree(Kind::ListItem, [lit])
            })
            .collect();

        let linked = self.block(linked);
        let linked = self.entry("linked", RitoType::container(PropertyKind::String), linked);

        let entries = bin
            .objects
            .values()
            .map(|obj| self.bin_object_to_cst(obj))
            .collect();

        let entries = self.block(entries);
        let entries = self.entry(
            "entries",
            RitoType::map(PropertyKind::Hash, PropertyKind::Embedded),
            entries,
        );

        self.nodes
            .get_mut(0)
            .unwrap()
            .children
            .extend([comment, type_entry, version, linked, entries]);
    }
}

#[cfg(test)]
mod test {
    use glam::Vec2;
    use ltk_meta::{
        property::{values, NoMeta},
        Bin, BinObject,
    };

    use super::*;
    use crate::print::CstPrinter;

    use bumpalo::Bump;

    // bin -> cst -> txt -> cst -> bin
    fn roundtrip(bin: Bin) {
        println!("bin: {bin:#?}");

        let bump = Bump::new();

        let builder = Builder::new(&bump);
        let (cst, buf) = builder.build(&bin);

        let mut str = String::new();
        cst.print(&mut str, &buf);

        println!("cst:\n{str}");

        let mut str = String::new();

        CstPrinter::new(&buf, &mut str, Default::default())
            .print(&cst)
            .unwrap();
        println!("RITOBIN:\n{str}");

        let cst2 = Cst::parse(&bump, &str);
        assert!(
            cst2.root().errors.is_empty(),
            "errors parsing ritobin - {:#?}",
            cst2.root().errors
        );
        let (bin2, errors) = cst2.build_bin(&str);

        assert!(
            errors.is_empty(),
            "errors building tree from reparsed ritobin - {errors:#?}"
        );

        pretty_assertions::assert_eq!(bin2, bin);
    }

    #[test]
    fn null() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::None::default())
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn numerics() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::U64::new(12))
                        .property(0x2, values::U32::new(23))
                        .property(0x3, values::U16::new(34))
                        .property(0x4, values::U8::new(45))
                        .property(0x11, values::I64::new(-12))
                        .property(0x22, values::I32::new(-23))
                        .property(0x33, values::I16::new(-34))
                        .property(0x44, values::I8::new(45))
                        .property(0x66, values::Hash::new(123123))
                        .property(0x99, values::F32::new(-45.45345))
                        .property(0x98, values::F32::new(199999.))
                        .build(),
                )
                .build(),
        );
    }
    #[test]
    fn vectors_colors_and_matrices() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::Vector2::new(glam::Vec2::new(0.1, -65.0)))
                        .property(0x2, values::Vector3::new(glam::Vec3::new(1000., -0.0, 2.)))
                        .property(
                            0x3,
                            values::Vector4::new(glam::Vec4::new(0.1, -65.0, 100.0, 481.)),
                        )
                        .property(
                            0x4,
                            values::Color::new(ltk_primitives::Color {
                                r: 123,
                                g: 255,
                                b: 2,
                                a: 5,
                            }),
                        )
                        .property(
                            0x5,
                            values::Matrix44::new(glam::Mat4::from_cols_array(&[
                                0.1, 0.5, 0.7, 0.9, 10.0, 11.2, 13.8, 15.3, 19.52, -0.123, -55.11,
                                -13.005, 23.0, 99.02, 101.1, 500.0,
                            ])),
                        )
                        .build(),
                )
                .build(),
        );
    }
    #[test]
    fn bool_bitbool() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::Bool::new(true))
                        .property(0x2, values::Bool::new(false))
                        .property(0x3, values::BitBool::new(true))
                        .property(0x4, values::BitBool::new(false))
                        .build(),
                )
                .build(),
        );
    }
    #[test]
    fn string() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x44444444, values::String::from("hello"))
                        .build(),
                )
                .build(),
        );
    }
    #[test]
    fn hashes() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::Hash::new(123123))
                        .property(0x2, values::Hash::new(u32::MAX))
                        .property(0x3, values::ObjectLink::new(123123))
                        .property(0x4, values::ObjectLink::new(u32::MAX))
                        .property(0x5, values::WadChunkLink::new(123123))
                        .property(0x6, values::WadChunkLink::new(u64::MAX))
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn struct_and_embedded() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(
                            0x91,
                            values::Struct {
                                class_hash: 0x123.into(),
                                meta: Default::default(),
                                properties: [
                                    (0x1.into(), values::U64::new(5).into()),
                                    (0x2.into(), values::U64::new(10).into()),
                                ]
                                .into_iter()
                                .collect(),
                            },
                        )
                        .property(
                            0x92,
                            values::Embedded(values::Struct {
                                class_hash: 0x234.into(),
                                meta: Default::default(),
                                properties: [
                                    (0x2.into(), values::U64::new(5).into()),
                                    (0x3.into(), values::U64::new(10).into()),
                                ]
                                .into_iter()
                                .collect(),
                            }),
                        )
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn lists() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(
                            0x9191919,
                            values::Container::new(vec![
                                values::U64::new(5),
                                values::U64::new(6),
                                values::U64::new(7),
                            ]),
                        )
                        .property(
                            0x9191918,
                            values::UnorderedContainer(values::Container::new(vec![
                                values::U32::new(5),
                                values::U32::new(6),
                                values::U32::new(7),
                            ])),
                        )
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn map() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xfeeb1e, 0x111)
                        .property(
                            0x1,
                            values::Map::new(
                                PropertyKind::String,
                                PropertyKind::U64,
                                vec![(
                                    values::String::from("asdasd").into(),
                                    values::U64::new(1).into(),
                                )],
                            )
                            .unwrap(),
                        )
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn optional() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::<NoMeta>::builder(0xDEADBEEF, 0x12344321)
                        .property(
                            0x1,
                            values::Optional::new(PropertyKind::Vector2, None).unwrap(),
                        )
                        .property(
                            0x2,
                            values::Optional::new(
                                PropertyKind::Vector2,
                                Some(values::Vector2::new(Vec2::new(0.1, -5.0)).into()),
                            )
                            .unwrap(),
                        )
                        .build(),
                )
                .build(),
        );
    }
}
