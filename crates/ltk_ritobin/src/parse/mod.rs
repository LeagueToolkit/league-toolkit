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
    use crate::typecheck::visitor::TypeChecker;

    use super::*;
    #[test]
    fn smoke_test() {
        let text = r#"
EmitterName: string = "EyeTrail1"
map: map[string, string] = 2
2

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

        eprintln!("{str}\n====== type errors: ======\n");
        for err in checker.into_diagnostics() {
            eprintln!("{:?}: {:#?}", &text[err.span], err.diagnostic);
        }
    }
}
