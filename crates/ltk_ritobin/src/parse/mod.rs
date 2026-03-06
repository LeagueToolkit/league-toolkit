//! Parser for ritobin text format with CST output for better error reporting.

pub mod cst;

pub mod error;
pub use error::*;

pub mod parser;
pub mod real;
pub mod tokenizer;
pub use tokenizer::{Token, TokenKind};

pub mod impls;

mod span;
pub use span::Span;

pub fn parse(text: &str) -> cst::Cst {
    let tokens = tokenizer::lex(text);
    let mut p = parser::Parser::new(text, tokens);
    impls::file(&mut p);
    p.build_tree()
}

#[cfg(test)]
mod test {
    use ltk_meta::{property::values, Bin, BinObject, BinProperty, PropertyValueEnum};

    use crate::typecheck::visitor::TypeChecker;

    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
entries: map[hash,embed] = {
    "myPath" = VfxEmitter {
        a: list[vec2] = {
            {2 2} 
        }
        e: list[embed] = {
            Foo {
                a: string = ""
            }
        }
    }
}
"#;
        let cst = parse(text);
        let errors = cst::FlatErrors::walk(&cst);

        let mut str = String::new();
        cst.print(&mut str, 0, text);
        eprintln!("text len: {}", text.len());
        eprintln!("{str}\n====== errors: ======\n");
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.kind);
        }

        let mut checker = TypeChecker::new(text);
        cst.walk(&mut checker);

        let (mut roots, errors) = checker.into_parts();

        eprintln!("{str}\n====== type errors: ======\n");
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.diagnostic);
        }

        eprintln!("{roots:#?}");
        let objects = roots
            .swap_remove("entries")
            .map(|v| {
                let PropertyValueEnum::Map(map) = v else {
                    panic!("entries must be map");
                };
                map.into_entries().into_iter().filter_map(|(key, value)| {
                    let path_hash = match &key {
                        PropertyValueEnum::Hash(h) => **h,
                        _ => return None,
                    };

                    if let PropertyValueEnum::Embedded(values::Embedded(struct_val)) = value {
                        Some(BinObject {
                            path_hash,
                            class_hash: struct_val.class_hash,
                            properties: struct_val.properties.clone(),
                        })
                    } else {
                        None
                    }
                })
            })
            .expect("no 'entries' entry");

        let tree = Bin::new(objects, [""]);
        eprintln!("{tree:#?}");
    }
}
