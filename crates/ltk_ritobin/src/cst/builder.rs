use std::fmt::{LowerHex, Write};

use ltk_meta::{property::values, Bin, BinObject, BinProperty, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{Child, Cst, Kind},
    parse::{Span, Token, TokenKind as Tok},
    typecheck::visitor::{PropertyValueExt, RitoType},
    HashProvider, RitobinName as _,
};

pub struct Builder<H: HashProvider> {
    buf: String,
    hashes: H,
}

pub fn tree(kind: Kind, children: Vec<Child>) -> Child {
    Child::Tree(Cst {
        span: Span::default(),
        kind,
        children,
        errors: vec![],
    })
}
pub fn token(kind: Tok) -> Child {
    Child::Token(Token {
        kind,
        span: Span::default(),
    })
}

impl Default for Builder<()> {
    fn default() -> Self {
        Self::new(())
    }
}

impl<H: HashProvider> Builder<H> {
    pub fn new(hashes: H) -> Self {
        Self {
            buf: String::new(),
            hashes,
        }
    }

    pub fn build(&mut self, bin: &Bin) -> Cst {
        self.bin_to_cst(bin)
    }

    /// Get a reference to the underlying text buffer all Cst's built by this builder reference in
    /// their spans.
    pub fn text_buffer(&self) -> &str {
        &self.buf
    }
    /// Get the underlying text buffer all Cst's built by this builder reference in
    /// their spans.
    pub fn into_text_buffer(self) -> String {
        self.buf
    }
}

fn hex_fmt<T: LowerHex>(v: T) -> String {
    format!("0x{v:x}")
}

impl<H: HashProvider> Builder<H> {
    fn number(&mut self, v: impl AsRef<str>) -> Child {
        tree(Kind::Literal, vec![self.spanned_token(Tok::Number, v)])
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

    fn string(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::String, v)
    }

    fn hex_lit(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::HexLit, v)
    }

    fn hash_hash_lit(&mut self, h: u32) -> Child {
        match self.hashes.lookup_hash(h).map(|h| format!("\"{h}\"")) {
            Some(h) => self.spanned_token(Tok::String, h),
            None => self.spanned_token(Tok::HexLit, hex_fmt(h)),
        }
    }
    fn hash_type_lit(&mut self, h: u32) -> Child {
        match self.hashes.lookup_type(h).map(|h| h.to_string()) {
            Some(h) => self.spanned_token(Tok::Name, h),
            None => self.spanned_token(Tok::HexLit, hex_fmt(h)),
        }
    }
    fn hash_field_lit(&mut self, h: u32) -> Child {
        match self.hashes.lookup_field(h).map(|h| h.to_string()) {
            Some(h) => self.spanned_token(Tok::Name, h),
            None => self.spanned_token(Tok::HexLit, hex_fmt(h)),
        }
    }
    fn hash_entry_lit(&mut self, h: u32) -> Child {
        match self.hashes.lookup_entry(h).map(|h| format!("\"{h}\"")) {
            Some(h) => self.spanned_token(Tok::String, h),
            None => self.spanned_token(Tok::HexLit, hex_fmt(h)),
        }
    }

    fn block(&self, children: Vec<Child>) -> Child {
        tree(
            Kind::Block,
            [vec![token(Tok::LCurly)], children, vec![token(Tok::RCurly)]].concat(),
        )
    }

    fn bool(&self, v: bool) -> Child {
        tree(
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

            PropertyValueEnum::U8(n) => self.number(n.to_string()),
            PropertyValueEnum::U16(n) => self.number(n.to_string()),
            PropertyValueEnum::U32(n) => self.number(n.to_string()),
            PropertyValueEnum::U64(n) => self.number(n.to_string()),
            PropertyValueEnum::I8(n) => self.number(n.to_string()),
            PropertyValueEnum::I16(n) => self.number(n.to_string()),
            PropertyValueEnum::I32(n) => self.number(n.to_string()),
            PropertyValueEnum::I64(n) => self.number(n.to_string()),
            PropertyValueEnum::F32(n) => self.number(n.to_string()),
            PropertyValueEnum::Vector2(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| tree(Kind::ListItem, vec![self.number(v.to_string())]))
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Vector3(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| tree(Kind::ListItem, vec![self.number(v.to_string())]))
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Vector4(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| tree(Kind::ListItem, vec![self.number(v.to_string())]))
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Matrix44(v) => {
                let items = v
                    .transpose() // ritobin text stores matrices row-major, glam::Mat4 is column-major.
                    .to_cols_array_2d()
                    .iter()
                    .flat_map(|v| {
                        [
                            tree(Kind::ListItem, vec![self.number(v[0].to_string())]),
                            tree(Kind::ListItem, vec![self.number(v[1].to_string())]),
                            tree(Kind::ListItem, vec![self.number(v[2].to_string())]),
                            tree(Kind::ListItem, vec![self.number(v[3].to_string())]),
                        ]
                    })
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::Color(v) => {
                let items = v
                    .to_array()
                    .iter()
                    .map(|v| tree(Kind::ListItem, vec![self.number(v.to_string())]))
                    .collect();
                self.block(items)
            }
            PropertyValueEnum::String(s) => tree(
                Kind::Literal,
                vec![token(Tok::Quote), self.string(&**s), token(Tok::Quote)],
            ),

            // hash/hash-likes
            PropertyValueEnum::Hash(h) => self.hash_hash_lit(**h),
            PropertyValueEnum::WadChunkLink(h) => self.hex_lit(hex_fmt(**h)),
            PropertyValueEnum::ObjectLink(h) => self.hash_hash_lit(**h),

            PropertyValueEnum::Container(container)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container)) => {
                let mut children = vec![token(Tok::LCurly)];

                for item in container.clone().into_items() {
                    children.push(tree(Kind::ListItem, vec![self.value_to_cst(&item)]));
                }

                children.push(token(Tok::RCurly));
                tree(Kind::TypeArgList, children)
            }
            PropertyValueEnum::Embedded(values::Embedded(s)) | PropertyValueEnum::Struct(s) => {
                let k = self.hash_type_lit(s.class_hash);
                let children = s
                    .properties
                    .iter()
                    .map(|(_k, v)| self.property_to_cst(v))
                    .collect();
                tree(Kind::Class, vec![k, self.block(children)])
            }

            PropertyValueEnum::Optional(optional) => {
                let children = match optional.clone().into_inner() {
                    Some(v) => vec![self.value_to_cst(&v)],
                    None => vec![],
                };
                self.block(children)
            }
            PropertyValueEnum::None(_) => tree(Kind::Literal, vec![token(Tok::Null)]),

            PropertyValueEnum::Map(map) => {
                let children = map
                    .entries()
                    .iter()
                    .map(|(k, v)| {
                        tree(
                            Kind::Entry,
                            vec![self.value_to_cst(k), token(Tok::Eq), self.value_to_cst(v)],
                        )
                    })
                    .collect();
                self.block(children)
            }
        }
    }

    fn entry(&self, key: Child, kind: Option<Child>, value: Child) -> Child {
        tree(
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
            let mut args = vec![
                token(Tok::LBrack),
                tree(
                    Kind::TypeArg,
                    vec![self.spanned_token(Tok::Name, sub.to_rito_name())],
                ),
            ];
            if let Some(sub) = rito_type.subtypes[1] {
                args.push(tree(
                    Kind::TypeArg,
                    vec![self.spanned_token(Tok::Name, sub.to_rito_name())],
                ));
            }
            args.push(token(Tok::RBrack));
            children.push(tree(Kind::TypeArgList, args));
        }

        tree(Kind::TypeExpr, children)
    }
    fn property_to_cst<M: Clone>(&mut self, prop: &BinProperty<M>) -> Child {
        let k = self.hash_field_lit(prop.name_hash);
        let t = self.rito_type(prop.value.rito_type());
        let v = self.value_to_cst(&prop.value);
        self.entry(k, Some(t), v)
    }

    fn class(&self, class_name: Child, items: Vec<Child>) -> Child {
        tree(Kind::Class, vec![class_name, self.block(items)])
    }

    fn bin_object_to_cst(&mut self, obj: &BinObject) -> Child {
        let k = self.hash_entry_lit(obj.path_hash);

        let class_hash = self.hash_type_lit(obj.class_hash);
        let class_values = obj
            .properties
            .iter()
            .map(|(k, v)| {
                let k = tree(Kind::EntryKey, vec![self.hash_field_lit(*k)]);
                let t = self.rito_type(v.value.rito_type());
                let v = self.value_to_cst(&v.value);
                self.entry(k, Some(t), v)
            })
            .collect();

        let value = self.class(class_hash, class_values);
        self.entry(k, None, value)
    }

    fn bin_to_cst(&mut self, bin: &Bin) -> Cst {
        let mut entries = Vec::new();

        for obj in bin.objects.values() {
            entries.push(self.bin_object_to_cst(obj));
        }

        let entries_key = self.spanned_token(Tok::Name, "entries");
        let entries_type = self.rito_type(RitoType {
            base: ltk_meta::PropertyKind::Map,
            subtypes: [Some(PropertyKind::Hash), Some(PropertyKind::Embedded)],
        });
        let entries = self.entry(
            tree(Kind::EntryKey, vec![entries_key]),
            Some(entries_type),
            tree(Kind::EntryValue, vec![self.block(entries)]),
        );

        Cst {
            kind: Kind::File,
            span: Span::default(),
            children: vec![entries],
            errors: vec![],
        }
    }
}

#[cfg(test)]
mod test {
    use ltk_meta::{property::values, Bin, BinObject};

    use super::*;
    use crate::print::CstPrinter;

    // bin -> cst -> txt -> cst -> bin
    fn roundtrip(bin: Bin) {
        println!("bin: {bin:#?}");

        let mut builder = Builder::default();
        let cst = builder.build(&bin);
        let buf = builder.text_buffer();

        let mut str = String::new();
        cst.print(&mut str, 0, buf);

        println!("cst:\n{str}");

        let mut str = String::new();

        CstPrinter::new(buf, &mut str, Default::default())
            .print(&cst)
            .unwrap();
        println!("RITOBIN:\n{str}");

        let cst2 = Cst::parse(&str);
        assert!(
            cst2.errors.is_empty(),
            "errors parsing ritobin - {:#?}",
            cst2.errors
        );
        let (bin2, errors) = cst2.build_bin(&str);

        assert!(
            errors.is_empty(),
            "errors building tree from reparsed ritobin - {errors:#?}"
        );

        pretty_assertions::assert_eq!(bin2, bin);
    }

    #[test]
    fn string() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
                        .property(0x44444444, values::String::from("hello"))
                        .build(),
                )
                .build(),
        );
    }

    #[test]
    fn null() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::None::default())
                        .build(),
                )
                .build(),
        );
        panic!();
    }

    #[test]
    fn numerics() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
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
                    BinObject::builder(0xDEADBEEF, 0x12344321)
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
    fn list() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
                        .property(0x1, values::String::from("hello"))
                        .property(0x2, values::U64::new(9))
                        .property(
                            0x9191919,
                            values::Container::new(vec![
                                values::U64::new(5),
                                values::U64::new(6),
                                values::U64::new(7),
                            ]),
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
                    BinObject::builder(0xfeeb1e, 0x111)
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
}
