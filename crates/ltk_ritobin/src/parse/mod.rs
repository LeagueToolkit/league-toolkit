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
    eprintln!("tokens: {tokens:#?}");
    let mut p = parser::Parser::new(tokens);
    impls::file(&mut p);
    p.build_tree()
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
linked: list[string, cock] = {
    "DATA/Characters/Yasuo/Yasuo.bin"
    "data/Yasuo_skin0_concat.bin"
    "data/Yasuo_skin0_StaticMat_proxy.bin"
}

"#;
        let cst = parse(text);
        let errors = cst::FlatErrors::walk(&cst);

        let mut str = String::new();
        cst.print(&mut str, 0, text);
        eprintln!("{str}\n====== errors: ======\n");
        for err in errors {
            eprintln!("{:?}: {:#?}", &text[err.span], err.kind);
        }
    }
}
