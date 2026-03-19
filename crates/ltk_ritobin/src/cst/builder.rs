use std::fmt::Write;

use ltk_meta::{property::values, Bin, BinObject, BinProperty, PropertyKind, PropertyValueEnum};

use crate::{
    cst::{Child, Cst, Kind},
    kind_to_type_name,
    parse::{Span, Token, TokenKind as Tok},
    typecheck::visitor::{PropertyValueExt, RitoType},
};

struct Builder {
    buf: String,
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

impl Builder {
    pub fn number(&mut self, v: impl AsRef<str>) -> Child {
        self.buf.write_str(v.as_ref()).unwrap();
        token(Tok::Number)
    }

    pub fn spanned_token(&mut self, kind: Tok, str: impl AsRef<str>) -> Child {
        let start = self.buf.len() as u32;
        self.buf.write_str(str.as_ref()).unwrap();
        let end = self.buf.len() as u32;
        Child::Token(Token {
            kind,
            span: Span::new(start, end),
        })
    }

    pub fn string(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::String, v)
    }

    pub fn name(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::Name, v)
    }

    pub fn hex_lit(&mut self, v: impl AsRef<str>) -> Child {
        self.spanned_token(Tok::HexLit, v)
    }

    pub fn block(&self, children: Vec<Child>) -> Child {
        tree(
            Kind::Block,
            [vec![token(Tok::LCurly)], children, vec![token(Tok::RCurly)]].concat(),
        )
    }

    fn value_to_cst<M: Clone>(&mut self, value: &PropertyValueEnum<M>) -> Child {
        match value {
            PropertyValueEnum::Bool(b) => tree(
                Kind::Literal,
                vec![token(if **b { Tok::True } else { Tok::False })],
            ),
            PropertyValueEnum::None(_) => {
                tree(Kind::Literal, vec![token(Tok::LCurly), token(Tok::RCurly)])
            }
            PropertyValueEnum::U8(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::U16(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::U32(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::U64(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::I8(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::I16(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::I32(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::I64(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::F32(n) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::Vector2(v) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::Vector3(v) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::Vector4(v) => tree(Kind::Literal, vec![token(Tok::Number)]),
            PropertyValueEnum::String(s) => tree(
                Kind::Literal,
                vec![token(Tok::Quote), self.string(&**s), token(Tok::Quote)],
            ),
            PropertyValueEnum::Container(container)
            | PropertyValueEnum::UnorderedContainer(values::UnorderedContainer(container)) => {
                let mut children = vec![token(Tok::LBrack)];

                for (i, item) in container.clone().into_items().enumerate() {
                    if i > 0 {
                        children.push(token(Tok::Comma));
                    }
                    children.push(self.value_to_cst(&item));
                }

                children.push(token(Tok::RBrack));
                tree(Kind::TypeArgList, children)
            }
            PropertyValueEnum::Matrix44(matrix44) => todo!(),
            PropertyValueEnum::Color(color) => todo!(),
            PropertyValueEnum::Hash(hash) => todo!(),
            PropertyValueEnum::WadChunkLink(wad_chunk_link) => todo!(),
            PropertyValueEnum::Struct(_) => todo!(),
            PropertyValueEnum::Embedded(embedded) => {
                let children = embedded
                    .0
                    .properties
                    .iter()
                    .map(|(k, v)| {
                        let k = self.spanned_token(Tok::HexLit, k.to_string());
                        let t = self.rito_type(v.value.rito_type());
                        let v = self.property_to_cst(v);
                        self.entry(k, Some(t), v)
                    })
                    .collect();
                tree(Kind::Class, vec![token(Tok::HexLit), self.block(children)])
            }
            PropertyValueEnum::ObjectLink(object_link) => todo!(),
            PropertyValueEnum::BitBool(bit_bool) => todo!(),
            PropertyValueEnum::Optional(optional) => {
                let children = match optional.clone().into_inner() {
                    Some(v) => vec![self.value_to_cst(&v)],
                    None => vec![],
                };
                self.block(children)
            }
            PropertyValueEnum::Map(map) => todo!(),
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
        let mut children = vec![self.spanned_token(Tok::Name, kind_to_type_name(rito_type.base))];

        if let Some(sub) = rito_type.subtypes[0] {
            let mut args = vec![
                token(Tok::LBrack),
                tree(
                    Kind::TypeArg,
                    vec![self.spanned_token(Tok::Name, kind_to_type_name(sub))],
                ),
            ];
            if let Some(sub) = rito_type.subtypes[1] {
                args.push(token(Tok::Comma));
                args.push(tree(
                    Kind::TypeArg,
                    vec![self.spanned_token(Tok::Name, kind_to_type_name(sub))],
                ));
            }
            args.push(token(Tok::RBrack));
            children.push(tree(Kind::TypeArgList, args));
        }

        tree(Kind::TypeExpr, children)
    }
    fn property_to_cst<M: Clone>(&mut self, prop: &BinProperty<M>) -> Child {
        let k = self.spanned_token(Tok::HexLit, prop.name_hash.to_string());
        let t = self.rito_type(prop.value.rito_type());
        let v = self.value_to_cst(&prop.value);
        self.entry(k, Some(t), v)
    }

    fn object_block(&mut self, obj: &BinObject) -> Child {
        let mut children = vec![token(Tok::LCurly)];

        for prop in obj.properties.values() {
            children.push(self.property_to_cst(prop));
        }

        children.push(token(Tok::RCurly));

        tree(Kind::Block, children)
    }

    fn class(&self, class_name: Child, items: Vec<Child>) -> Child {
        tree(Kind::Class, vec![class_name, self.block(items)])
    }

    fn bin_object_to_cst(&mut self, obj: &BinObject) -> Child {
        let k = self.hex_lit(format!("0x{:x}", obj.path_hash));

        let class_hash = self.hex_lit(format!("0x{:x}", obj.class_hash));
        let class_values = obj
            .properties
            .iter()
            .map(|(k, v)| {
                let k = tree(Kind::EntryKey, vec![self.hex_lit(format!("0x{k:x}"))]);
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

pub fn bin_to_cst(bin: &Bin) -> (String, Cst) {
    let mut builder = Builder { buf: String::new() };
    let cst = builder.bin_to_cst(bin);

    (builder.buf, cst)
}

#[cfg(test)]
mod test {
    use ltk_meta::{property::values, Bin, BinObject};

    use crate::{cst::builder::bin_to_cst, parse::parse, print::CstPrinter};

    fn roundtrip(bin: Bin) {
        println!("bin: {bin:#?}");

        let (buf, cst) = bin_to_cst(&bin);

        let mut str = String::new();
        cst.print(&mut str, 0, &buf);

        println!("cst:\n{str}");

        let mut str = String::new();

        CstPrinter::new(&buf, &mut str, Default::default())
            .print(&cst)
            .unwrap();
        println!("RITOBIN:\n{str}");

        let cst2 = parse(&str);
        let (bin2, errors) = cst2.build_bin(&str);

        pretty_assertions::assert_eq!(bin2, bin);
    }

    #[test]
    fn simple() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
                        .property(0x44444444, values::String::from("hello"))
                        .build(),
                )
                .build(),
        );

        panic!();
    }
    #[test]
    fn list() {
        roundtrip(
            Bin::builder()
                .object(
                    BinObject::builder(0xDEADBEEF, 0x12344321)
                        .property(0x44444444, values::String::from("hello"))
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

        panic!();
    }
}
