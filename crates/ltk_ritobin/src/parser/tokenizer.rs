use crate::Span;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[rustfmt::skip]
pub enum TokenKind {
  Unknown, Eof,

  LParen, RParen, LCurly, RCurly,
  LBrack, RBrack,
  Eq, Comma, Colon,
  Minus, Star, Slash,
  Quote,

  String, UnterminatedString,


  True, False,

  Name, Int,
}

#[derive(Clone, Copy, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}
pub fn lex(mut text: &str) -> Vec<Token> {
    use TokenKind::*;
    let punctuation = (
        "( ) { } [ ] = , : - * /",
        [
            LParen, RParen, LCurly, RCurly, LBrack, RBrack, Eq, Comma, Colon, Minus, Star, Slash,
        ],
    );

    let keywords = ("true false", [True, False]);

    let source = text;

    let mut result = Vec::new();
    while !text.is_empty() {
        if let Some(rest) = trim(text, |it| it.is_ascii_whitespace()) {
            text = rest;
            continue;
        }
        let text_orig = text;
        let mut kind = 'kind: {
            for (i, symbol) in punctuation.0.split_ascii_whitespace().enumerate() {
                if let Some(rest) = text.strip_prefix(symbol) {
                    text = rest;
                    break 'kind punctuation.1[i];
                }
            }
            if let Some(rest) = trim(text, |it| it.is_ascii_digit()) {
                text = rest;
                break 'kind Int;
            }

            if let Some(rest) = text.strip_prefix(['\'', '"']) {
                text = rest;
                let mut skip = false;
                loop {
                    let Some(c) = text.chars().next() else {
                        break 'kind UnterminatedString;
                    };

                    text = &text[c.len_utf8()..];
                    match c {
                        '\\' => {
                            skip = true;
                        }
                        '\'' | '"' => match skip {
                            true => {
                                skip = false;
                            }
                            false => {
                                break 'kind String;
                            }
                        },
                        '\n' | '\r' => break 'kind UnterminatedString,
                        _ => {}
                    }
                }
            }
            if let Some(rest) = trim(text, name_char) {
                text = rest;
                break 'kind Name;
            }

            let error_index = text
                .find(|it: char| it.is_ascii_whitespace())
                .unwrap_or(text.len());
            text = &text[error_index..];
            Unknown
        };
        assert!(text.len() < text_orig.len());
        let token_text = &text_orig[..text_orig.len() - text.len()];

        let start = source.len() - text_orig.len();
        let end = source.len() - text.len();

        let span = Span {
            start: start as u32,
            end: end as u32,
        };
        if kind == Name {
            for (i, symbol) in keywords.0.split_ascii_whitespace().enumerate() {
                if token_text == symbol {
                    kind = keywords.1[i];
                    break;
                }
            }
        }
        result.push(Token { kind, span });
        // if kind == Quote {
        //     eprintln!("[lex] pushed quote!");
        //     let span = match find_string_closer(text) {
        //         Some(close_idx) => {
        //             let start = (source.len() - text.len()) as u32;
        //             Span {
        //                 start,
        //                 end: start + close_idx as u32,
        //             }
        //         }
        //         // &text[..close_idx],
        //         None => {
        //             let start = (source.len() - text.len()) as u32;
        //             Span {
        //                 start,
        //                 end: start + text.len() as u32,
        //             }
        //         }
        //     };
        //     eprintln!("[lex] token text: {token_text:?}");
        //     result.push(Token { kind: String, span });
        //     text = &source[span.end as _..];
        //     result.push(Token {
        //         kind: Quote,
        //         span: Span {
        //             start: span.end,
        //             end: span.end + 1,
        //         },
        //     });
        //     eprint!("[lex] text: {text:?}");
        //     text = &source[(span.end as usize + 1).min(source.len().saturating_sub(1))..];
        //     eprintln!(" -> {text:?}");
        // }
    }
    return result;

    fn name_char(c: char) -> bool {
        matches!(c, '_' | 'a'..='z' | 'A'..='Z' | '0'..='9')
    }

    fn trim(text: &str, predicate: impl std::ops::Fn(char) -> bool) -> Option<&str> {
        let index = text.find(|it: char| !predicate(it)).unwrap_or(text.len());
        if index == 0 {
            None
        } else {
            Some(&text[index..])
        }
    }

    fn find_string_closer(text: &str) -> Option<usize> {
        let mut skip = false;
        for (i, char) in text.char_indices() {
            if skip {
                skip = false;
                continue;
            }
            match char {
                '\\' => skip = true,
                '\'' | '"' => return Some(i),
                _ => {}
            }
        }
        None
    }
}
